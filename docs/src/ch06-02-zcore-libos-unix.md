# zCore实现的LibOS对UNIX模式的支持

### zCore on riscv64的LibOS unix模式的入口
LibOS unix模式的入口在linux-loader `main.rs:main()` <br>
初始化包括kernel_hal_unix，Host文件系统，elf应用程序加载，其中载入elf应用程序的过程与zCore bare模式一样。<br>
重点工作应该在kernel_hal_unix中的内核态与用户态互相切换的处理。

### zCore kernel_hal_unix的实现
kernel_hal_unix主要实现了kernel-hal声明的接口函数； 其中当target os是MacOS时，构建Segmentation Fault时SIGSEGV信号的处理函数，当代码尝试使用fs寄存器时会触发信号； 

* 为什么要注册SIGSEGV信号处理函数呢？ 

由于 macOS 用户程序无法修改 fs 寄存器(根据wrj的说明），当运行相关指令时会访问非法内存地址触发Segmentation Fault。<br>
故实现段错误信号处理函数，并在其中动态修改用户程序指令，将 fs 改为 gs <br>

kernel_hal_unix还构造了**进入用户态**所需的`run_fncall()` -> `syscall_fn_return()`<br>
而用户程序需要调用`syscall_fn_entry()`来**返回内核态** <br>

Linux-x86_64平台运行时，用户态和内核态之间的切换运用了 fs base 寄存器；<br>
Linux 和 macOS 下如何分别通过系统调用设置 `fsbase` / `gsbase` <br>
这个转换过程调用到了trapframe库，x86_64和aarch64有对应实现，而riscv没有，需要手动来实现；

* 关于fs寄存器 

查找了fs寄存器一般会用于寻址TLS，每个线程有它自己的fs base地址；<br>
fs寄存器被glibc定义为存放tls信息，结构体tcbhead_t就是用来描述tls；<br>
进入用户态前，将内核栈指针保存在内核 glibc 的 TLS 区域中。

可参考一个运行时程序的代码转换工具：https://github.com/DynamoRIO/dynamorio/issues/1568#issuecomment-239819506

### LibOS内核态与用户态的切换

Linux x86_64中，fs寄存器是用户态程序无法设置的，只能通过系统调用进行设置；<br>
例如clone系统调用，通过`arch_prctl`来设置`fs`寄存器；指向的struct pthread，glibc中，其中的首个结构是`tcbhead_t`

* 计算tls结构体偏移

经过实际试验，x86_64平台，int型：4节，指针类型：8节，无符号长整型：8节；
riscv64平台，int型： 4节，指针类型：8节，无符号长整型：8节；
计算tls偏移量时，注意下，在musl中，aarch64和riscv64架构有#define TLS_ABOVE_TP，而x86_64无此定义

* 关于类似LibOS的Linux user mode (UML)

["No, UML works only on x86 and x86_64."](https://sourceforge.net/p/user-mode-linux/mailman/message/32782012/)
