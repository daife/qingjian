//! 识别文字插入；异步取得写锁后再核验目标，避免把结果写到切换后的输入框。

use std::mem::ManuallyDrop;
use windows::Win32::UI::TextServices::{
    INSERT_TEXT_AT_SELECTION_FLAGS, ITfContext, ITfEditSession, ITfEditSession_Impl,
    ITfInsertAtSelection, TF_AE_END, TF_ANCHOR_END, TF_ES_READWRITE, TF_SELECTION,
    TF_SELECTIONSTYLE,
};
use windows::core::{Interface, Result, implement};

#[implement(ITfEditSession)]
struct RecognitionSession {
    context: ITfContext,

    text: Vec<u16>,
}

impl ITfEditSession_Impl for RecognitionSession_Impl {
    fn DoEditSession(&self, cookie: u32) -> Result<()> {
        if !crate::com::service::recognition::matches(&self.context) {
            return Ok(());
        }
        let insert: ITfInsertAtSelection = self.context.cast()?;
        let range = unsafe {
            insert.InsertTextAtSelection(cookie, INSERT_TEXT_AT_SELECTION_FLAGS(0), &self.text)?
        };
        unsafe { range.Collapse(cookie, TF_ANCHOR_END)? };
        let selection = TF_SELECTION {
            range: ManuallyDrop::new(Some(range)),
            style: TF_SELECTIONSTYLE {
                ase: TF_AE_END,
                fInterimChar: false.into(),
            },
        };
        let result = unsafe {
            self.context
                .SetSelection(cookie, std::slice::from_ref(&selection))
        };
        drop(ManuallyDrop::into_inner(selection.range));
        if result.is_err() {
            crate::com::log::log("辅助功能：识别结果插入失败，结果仍在剪贴板");
        }
        result
    }
}

pub(crate) fn request(context: &ITfContext, client: u32, text: String) -> Result<()> {
    let session = RecognitionSession {
        context: context.clone(),
        text: text.encode_utf16().collect(),
    };
    super::update::request(context, client, session.into(), TF_ES_READWRITE)
}
