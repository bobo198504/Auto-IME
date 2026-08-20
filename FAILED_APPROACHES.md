# 已确认不可用的方案（请勿再次使用）

以下方案都在本项目的调试中确认过不可用。除非找到能解决对应缺陷的新方法，否则不要重新采用。

## 1. 用 ImmSetOpenStatus 直接设置输入法“开关状态”

- 现象：微信/QQ 等应用方向相反、切换失效。
- 原因：`ImmSetOpenStatus` 需要先拿到当前进程的 HIMC（`ImmGetContext`），跨进程读其它应用时拿不到，
  所以不是“开关语义”错，而是跨进程访问失败。冰凌五笔的 Ctrl+空格实际就是切换 open/close。
- 结论：不要用 IMM 的 `ImmSetOpenStatus` 跨进程设。跨进程等价做法是
  `SendMessage(ImmGetDefaultIMEWnd(hwnd), WM_IME_CONTROL, IMC_SETOPENSTATUS(0x006), 1或0)`。

## 2. 用 ImmGetContext + ImmGetConversionStatus / ImmGetOpenStatus 读取当前中英文状态

- 现象：点击规则控件会来回切换中英文（Explorer、Reaper 全乱）。
- 原因：`ImmGetContext` 只能取“当前进程”的 IMM 上下文，跨进程读其它应用的窗口拿不到真实状态。
  已分别试过用顶层窗口 hwnd 和 GetGUIThreadInfo 的焦点控件 hwnd，均因跨进程失效。
- 结论：不要再用 `ImmGetContext` 跨进程读。正确做法是
  `SendMessage(ImmGetDefaultIMEWnd(hwnd), WM_IME_CONTROL, IMC_GETOPENSTATUS(0x005), 0)`，
  返回值非 0 即中文、0 即英文。本项目实测冰凌五笔的 Ctrl+空格改变的是 open 状态，
  而 `IMC_GETCONVERSIONMODE(0x001)` 一直返回 1，不能反映中英文（见 `win32.rs`）。

## 3. 状态表按进程（pid）聚合

- 现象：同一进程内多个线程/窗口的输入法状态互相干扰，切换不对。
- 原因：Windows 输入法状态是 per-thread 的，按进程聚合和系统真实行为不一致。
- 结论：状态表应保持按线程（thread_id）记录。

## 4. 用 GetAsyncKeyState 轮询监听手动切换

- 现象：手动切换有时能跟踪、有时漏掉，导致后续切换方向反（微信老问题之一）。
- 原因：20ms 轮询会漏检快速按键，且无法可靠区分“程序模拟的切换”和“用户手动切换”。
- 结论：不可靠。已从当前实现中移除；现改为在每次焦点/点击命中时直接读取真实 IME 状态，
  不再靠轮询猜测。若要事件级跟踪切换键，可改用 WH_KEYBOARD_LL 低级键盘钩子（尚未实现）。
