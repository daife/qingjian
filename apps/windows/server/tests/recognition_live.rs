//! 显式开启的网络验证，只上传外部参考项目的合成样本；日常测试不联网、不录音。

#[test]
#[ignore = "requires QINGJIAN_RECOGNITION_FIXTURES and uploads synthetic fixtures to Youdao"]
fn youdao_ocr_and_sixty_second_speech() {
    let root = std::path::PathBuf::from(
        std::env::var_os("QINGJIAN_RECOGNITION_FIXTURES")
            .expect("set fixture directory explicitly"),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let client = qingjian_recognize::Recognizer::new().unwrap();
        let text = client
            .ocr(std::fs::read(root.join("ocr.png")).unwrap())
            .await
            .unwrap();
        assert!(
            text.to_ascii_lowercase().contains("youdao"),
            "synthetic OCR fixture was not recognized"
        );
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        let fixture = std::fs::read(root.join("speech-en.wav")).unwrap();
        // 参考 WAV 带 18 字节 fmt chunk，按 RIFF chunk 长度定位，不假设 44 字节头。
        let mut chunk = 12;
        let pcm = loop {
            let size =
                u32::from_le_bytes(fixture[chunk + 4..chunk + 8].try_into().unwrap()) as usize;
            if &fixture[chunk..chunk + 4] == b"data" {
                break &fixture[chunk + 8..chunk + 8 + size];
            }
            chunk += 8 + size + size % 2;
        };
        assert!(!pcm.is_empty(), "speech fixture must contain samples");
        let mut speech = fixture[..36].to_vec();
        let length = 16_000 * 60 * 2;
        speech[4..8].copy_from_slice(&(36 + length as u32).to_le_bytes());
        speech[16..20].copy_from_slice(&16u32.to_le_bytes());
        speech.extend_from_slice(b"data");
        speech.extend_from_slice(&(length as u32).to_le_bytes());
        while speech.len() < length + 44 {
            let remaining = length + 44 - speech.len();
            speech.extend_from_slice(&pcm[..pcm.len().min(remaining)]);
        }
        let result = client.speech(speech, "en").await.unwrap();
        assert!(
            result.len() > 100,
            "sixty-second speech result was unexpectedly short"
        );
    });
}
