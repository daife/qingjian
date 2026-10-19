//! Server 自启动请求的结果，供断连期间的按键放行判断使用。

/// 连不上 Server 时尝试拉起它的结果，调用方据此决定断连期间还吞不吞拼音键。
pub(in crate::com::service) enum LaunchOutcome {
    /// 拉起请求刚发出：Server 一两秒内就该监听，宽限期内继续吞「可能是拼音」的键。
    Requested,
    /// 别处已在拉（本进程冷却未过 / 另一进程握着互斥体）：Server 也在路上，与 [`Self::Requested`] 同待遇，
    /// 只是宽限不重新计时（冷却每 5 秒就会回到这个分支，重置宽限会让「超时放行」永远轮不到）。
    InFlight,
    /// 这个宿主里起不出 Server（完整性级别不对 / 安装程序占用 / 找不到 exe）：短期不会有 Server，
    /// 继续吞键就是死键盘，调用方应直接放行字母。
    Refused,
}
