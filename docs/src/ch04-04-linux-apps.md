# zCore对Linux用户程序的兼容

zCore 是用 Rust 语言实现的兼容 Linux 内核。它支持x86、riscv这几种指令集，能够运行比较丰富的应用程序。<br>
zCore 项目采用了清晰的分层结构，同时复用 Zircon 微内核的内核对象实现了 Linux 内核的部分功能（如内存管理和进程管理）。<br>
目前 zCore 中的 linux 模块已经能够运行基础的 Busybox 等小程序。

### 在 LibOS 与 QEMU 中实现 stdin

在一开始还不太了解 LibOS 与 QEMU 区别的时候，以为 LibOS 只是一个更方便的，可以不需要 QEMU 就能运行操作系统的平台，所以我认为把功能在 LibOS 中实现的话 QEMU 里也可以用，但是事实并不是这样。我一开始在实现的 stdin 只能在 LibOS 中使用，到了 QEMU 里完全没有任何反应，后来经过王润基学长的讲解我才知道<br>
***LibOS 与在 QEMU 中运行的区别***：

- LibOS 是运行在用户态的操作系统，系统调用的实现方式从 `syscall` 指令改成了函数调用，此外需要与硬件打交道的地方也都改为与 rust 的 std crate 进行交互
- QEMU 环境则是与我们理解的裸机一样，操作系统与 QEMU 模拟出的硬件交互，与在真机上跑几乎没有区别

在 QEMU 中实现 stdin 时我还犯了一些低级错误：我以为 zCore 不能从 `trap_handler` 接收中断，结果最后才知道这一函数仅接收内核态中断，用户态中断是在另一个地方接收的，在这上面浪费了几天时间

### 在 LibOS 与 QEMU 中移植 shell

其实在 LibOS 中把 stdin 和 `sys_poll` 写好后，shell 就可以勉强运行了。之所以说勉强运行，是因为由于 `fork` 的限制，在 LibOS 中仅能使用 `sys_vfork` 而不能使用 `sys_fork`，而 AlpineLinux minirootfs 中的 shell (`busybox`) 启动外部程序必须使用 `sys_fork`，所以 shell 只能执行内置的，如 `cd`, `pwd` 之类的命令

而 LibOS 中的 shell 真正移植成功实际上是王润基学长的功劳，他发现 `busybox` 编译时有一个 Force-NOMMU 参数，使用该参数编译后 shell 启动外部程序则会使用 `sys_vfork`，有了这一版本的 `busybox`，就宣告着 LibOS 中的 shell 移植成功了

在 QEMU 中，其实也是写好改进的 stdin 之后，shell 就可以勉强运行了。这里的勉强运行跟 LibOS 中还不太一样，QEMU 中的 shell 可以执行外部命令，但是执行几次后就会因为 `sys_wait4` 而阻塞，经过一段时间的折腾我解决了这个问题并提交了 PR，但是在解决这一问题后我又偶然发现 LibOS 中的 shell 坏掉了...这个问题到现在还没有解决...

### 在 LibOS 与 QEMU 中移植 GCC

在 GCC 上面花了不少时间，因为其它的程序都可以直接从 Alpine Linux 中直接复制出来就可以运行，但由于编译 GCC 的时候没有开启 PIE-enabled 参数，导致 GCC 是 PIE-disabled 的，这样的程序在 zCore 中不能运行，所以折腾 GCC 也用了好久，没有找到解决方法，最终选择了在 Alpine Linux 中重新编译 `musl-gcc`，并全局添加 `-pie -fpie` 参数，这样最终编译出的 GCC 就可以在 zCore 中正常运行了

GCC 在补充了一些系统调用之后就可以编译出 `*.o` 的中间结果了，但是会跟 QEMU 中的 shell 一样，会因为 `sys_wait4` 而阻塞，区别是，shell 的阻塞是随机发生的，而 GCC 的阻塞是必然的，所以其实我本打算先放着 shell 不管的，但是 GCC 对此也有需求，所以就不得不解决了，也算是一举两得

### 在 LibOS 与 QEMU 中移植 Rust 工具链 

（待完善）在 Rust 工具链上面花了最多的时间，但是最终还是没有成功。但不能说是没有进展，也是向前推动了一小步的。

`rustc` 在一开始会死循环调用 `sys_poll`，这一问题卡了我们很久，最后在使用 `strace` 跟踪并查文档的时候偶然发现，`sys_poll` 需要写回一个参数，而我从 rCore 中搬过来的代码里面就没有，所以我在 rCore 与 zCore 中均提交了一个 PR 来解决这一问题

这之后在 LibOS 中 `rustc` 与编译无关的功能均可正常使用了，但是编译则会报段错误，而且出现的位置不定，这个一直没有解决

此外，`rustc` 在 QEMU 中运行则会报 OOM (out of memory)，我甚至把 zCore 的内存改成了 2G 都无法运行，王润基学长说需要修改 `sys_mmap`，但是这里的工作量太大，截止到活动结束我都没有完成，有点遗憾

### 在zCore 中 Linux 相关模块的单元测试

在开始之前，相关单元测试仅仅使用了运行 busybox 作为一个简单的测试，并且不检测用户程序执行的返回值；

在这段时间中，我们添加了大量基于用户态程序的单元测试，并完善了相应测试代码，补全了用户态返回值判断和自动编译测试用例的部分，使得现在可以简单地通过这样的方法使用 c 语言对系统调用进行单元测试：

1. 在 linux-syscall/test 文件夹里面编写 c 语言的测试程序，可以使用 assert 函数判断是否正确；
2. 在 linux-loader 的 main.rs 里面可以这样写：

   ```rust
   #[async_std::test]
   async fn test_pipe() {
       assert_eq!(test("/bin/testpipe1").await, 0);
   }
   ```

3. 运行 `make rootfs` 命令
4. run test

三个模块的单元测试覆盖率变化如下：

- linux-loader  74.63 -> 87.88
- linux-syscall 18.68 -> 61.55
- linux-object  41.84 -> 56.17

除此之外，linux 相关三个模块的文档均已补齐，并均能通过 `#[deny(missing_docs)]` 编译，主要参考linux相关文档；另外也参与了一点 libc-test 移植相关的工作。

参考：<br>
https://github.com/yunwei37/zcore_migration_notes
