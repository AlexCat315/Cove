# CoveSystem

`CoveFileSystem` 和 `CoveProcess` 可独立构建，不依赖 SDL、窗口、字体或 GPU。SDK、CLI 与 Studio 使用同一份实现。

```sh
./cove test System --no-parallel
```

`AtomicFile.write(_:to:access:)` 在目标目录创建暂存文件，再使用同卷替换发布。失败保留旧文件并清理暂存文件。Windows 仅对共享锁和锁冲突进行有限重试；原子可见性不等于断电后的持久性。`.ownerOnly` 在写入内容前设置 POSIX 权限或当前用户 ACL。

`PathIdentity` 解析现有祖先后比较路径；现有文件用文件身份比较，目录包含关系沿父目录检查，路径遍历在 POSIX、盘符及 UNC 根目录终止。`FileTrash.move(_:recoveryDirectory:)` 在 macOS 使用系统回收站，在其他平台移入调用方指定的恢复目录，失败不会永久删除。

`ProcessSession` 接受已解析的可执行文件和显式环境。`ToolLocator` 负责工具搜索与 Windows 环境变量大小写；权限、命令白名单和沙箱规则仍由调用方决定。每个会话只有一个事件消费者，可以遍历异步 `events` 或同步读取 `nextEvent(until:)`。stdout、stderr 在独立线程排空；只有读取线程关闭读取句柄。输出限额、输入限额和总超时分别约束资源，总超时包含进程退出后的管道排空。Windows 在恢复子进程前绑定 Job Object，并用句柄白名单限制管道继承；POSIX 使用进程组回收后代。同步操作只等待原生线程，不依赖 Swift 协作执行器。

文件操作可用于桌面和移动端。进程服务当前支持 macOS、Linux 和 Windows；iOS、Android 等其他目标保持可编译，调用启动接口时明确抛出 `ProcessError.unavailable`。

Windows 原生程序的参数直接按 CRT 规则传递。`.cmd`、`.bat` 需要 `cmd.exe`；为避免参数被解释成命令语法，批处理参数含引号、换行或 shell 元字符时会抛出 `ProcessError.invalidCommand`。需要传入任意参数的工具应绑定原生可执行文件。

测试用真实辅助进程覆盖短消息、交互输入、最终输出、取消以及父进程退出后后代仍持有管道的情形。底层适配仅在 `System/Sources` 中维护。
