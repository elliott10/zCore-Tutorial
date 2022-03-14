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

------
### 通过函数调用的上下文切换的实现fncall

#### 分析代码中定义的五个macro

|||
| - | - |
|SWITCH_TO_KERNEL_STACK | 让rsp(栈顶指针)指向kernel stack|

代码如下:
```
.macro SWITCH_TO_KERNEL_STACK
    mov rsp, fs:48          # rsp = kernel fsbase
    mov rsp, [rsp + 64]     # rsp = kernel stack
.endm
```
其中，rsp为栈顶指针
在`mov rsp, fs:48`后, FS与rsp寄存器如下:
![Figure 1](img/fncallfiles/SWITCH_TO_KERNEL_STACK.png)
<br>
在`mov rsp, [rsp + 64]`后, FS与rsp寄存器如下:
![Figure 2](img/fncallfiles/SWITCH_TO_KERNEL_STACK_2.png)

|||
|-|-|
|SAVE_KERNEL_STACK|将rsp的内容存进fs:64<br>- fs:64 (pthread.???)        = kernel stack|

代码如下:
```
.macro SAVE_KERNEL_STACK
    mov fs:64, rsp
.endm
```
即,将当前rsp寄存器中的内容作为kernel stack写入fs:48中

|||
|-|-|
|PUSH_USER_FSBASE|将fs:0(user fsbase)的内容压入栈|
```
.macro PUSH_USER_FSBASE
    push fs:0
.endm
```

|||
|-|-|
|SWITCH_TO_KERNEL_FSBASE|从ring3进入ring0,并且将fs寄存器所指向的fsbase由user fsbase切换为kernel fsbase|
代码如下:
```
.macro SWITCH_TO_KERNEL_FSBASE
    mov eax, 158            # SYS_arch_prctl
    mov edi, 0x1002         # SET_FS
    mov rsi, fs:48          # rsi = kernel fsbase
    syscall
.endm
```
其中, eax中储存了系统调用号158, 即SYS_arch_prctl, edi和rsi中储存了参数(0x1002, fs:48), 使用syscall命令执行系统调用, 作用是将fs:48中的地址(kernel fsbase)写入fs寄存器中

如图:
![Figure 3](img/fncallfiles/SWITCH_TO_KERNEL_FSBASE.png)


|||
|-|-|
|POP_USER_FSBASE|还原user fsbase|

```
.macro POP_USER_FSBASE
    mov rsi, [rsp + 18 * 8] # rsi = user fsbase
    mov rdx, fs:0           # rdx = kernel fsbase
    test rsi, rsi           # if [rsp + 18 * 8] is 0
    jnz 1f                  # if not 0, goto set
0:  lea rsi, [rdx + 72]     # rsi = init user fsbase
    mov [rsi], rsi          # user_fs:0 = user fsbase
    # if 0, user_fs:0 = init user fsbase

1:  mov eax, 158            # SYS_arch_prctl
    mov edi, 0x1002         # SET_FS
    syscall                 # set fsbase

    # if not 0, set FS to [rsp + 18 * 8]

    mov fs:48, rdx          # user_fs:48 = kernel fsbase

.endm
```

FS寄存器设置为user fsbase <br>

如果[rsp + 18 * 8]不为空，user fsbase = [rsp + 18 * 8] <br>

如果[rsp + 18 * 8]为空，user fsbase为初始user fsbase；将user_fs:48设置为kernel fsbase <br>

#### 定义syscall_fn_entry及syscall_fn_return, 保存上下文
syscall_fn_entry: 函数入口 <br>
syscall_fn_return: 函数返回 <br>
   
#### 测试程序验证
<table>
<tr><th>code</th><th>explanation</th></tr>

<tr>
<td><pre lang="rust">
#[cfg(test)]
mod tests {
    use crate::*;

    #[cfg(target_os = "macos")]
    global_asm!(".set _dump_registers, dump_registers");
</pre></td>

<td>
这里定义了dump_registers
</td>
</tr>

<tr>
<td><pre lang="rust">
    // Mock user program to dump registers at stack.
    global_asm!(
        r#"
.intel_syntax noprefix
dump_registers:
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rsp
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax
    add rax, 10
    add rbx, 10
    add rcx, 10
    add rdx, 10
    add rsi, 10
    add rdi, 10
    add rbp, 10
    add r8, 10
    add r9, 10
    add r10, 10
    add r11, 10
    add r12, 10
    add r13, 10
    add r14, 10
    add r15, 10
    call syscall_fn_entry
"#
    );

</pre></td>

<td>
这里是dump_registers的代码，程序的流程功能：<br>
1、将rax等16个通用寄存器的值压栈 <br>
2、每个通用寄存器的值加10 <br>
3、调用syscall_fn_entry <br>
</td>
</tr>

<tr>
<td><pre lang="rust">
#[test]
fn run_fncall() {
    extern "sysv64" {
        fn dump_registers();
    }
    let mut stack = [0u8; 0x1000];
    let mut cx = UserContext {
        general: GeneralRegs {
            rax: 0,
            rbx: 1,
            rcx: 2,
            rdx: 3,
            rsi: 4,
            rdi: 5,
            rbp: 6,
            rsp: stack.as_mut_ptr() as usize + 0x1000,
            r8: 8,
            r9: 9,
            r10: 10,
            r11: 11,
            r12: 12,
            r13: 13,
            r14: 14,
            r15: 15,
            rip: dump_registers as usize,
            rflags: 0,
            fsbase: 0, // don't set to non-zero garbage value
            gsbase: 0,
        },
        trap_num: 0,
        error_code: 0,
    };
</pre></td>

<td>
run_fncall的流程：<br>
1. 定义一个数组stack，长度0x1000，内容为0<br>
2. 定义一个UserContext结构cx，并且进行初始化，初始化之后数据如下图所示：<br>
<img src="fncallfiles/run_fncall/2.png"></img>

3. 由于rip的值为dump_register，因此此时会进入dump_register的代码：<br>
push寄存器和修改寄存器的值，stack的内容如下图所示：<br>
<img src="fncallfiles/run_fncall/3.png"></img>

此时，调用syscall_fn_entry，kernel stack的内容变化如下图所示：<br>
<img src="fncallfiles/run_fncall/4.png"></img>

<img src="fncallfiles/run_fncall/5.png"></img>

【注】：代码中两次pop rbx，是否笔误？
<td>
</tr>

<tr>
<td><pre lang="rust">
cx.run_fncall();
</pre></td>
<td>
4. 从dump_register返回后，调用run_fncall(&mut self)，代码为：
<pre>
{  
unsafe { syscall_fn_return(self); }
          self.trap_num = 0x100; 
self.error_code = 0; 
}
</pre>
调用syscall_fn_return，kernel stack的内容变化如下图所示：<br>
保存被调用者寄存器<br>
<img src="fncallfiles/run_fncall/6.png"></img>

恢复GeneralRegs<br>
<img src="fncallfiles/run_fncall/7.png"></img>
</td>
</tr>

<tr>
<td><pre>
// check restored registers
let general = unsafe { *(cx.general.rsp as *const GeneralRegs) };
assert_eq!(
    general,
        GeneralRegs {
            rax: 0,
            rbx: 1,
            rcx: 2,
            rdx: 3,
            rsi: 4,
            rdi: 5,
            rbp: 6,
            // skip rsp
            r8: 8,
            r9: 9,
            r10: 10,
            // skip r11
            r12: 12,
            r13: 13,
            r14: 14,
            r15: 15,
            ..general
        }
    );
</pre></td>
<td>
5. 检查恢复的寄存器<br>
<img src="fncallfiles/run_fncall/8.png"></img>
</td>
</tr>

<tr>
<td><pre>
// check saved registers
assert_eq!(
    cx.general,
    GeneralRegs {
        rax: 10,
        rbx: 11,
        rcx: 12,
        rdx: 13,
        rsi: 14,
        rdi: 15,
        rbp: 16,
        // skip rsp
        r8: 18,
        r9: 19,
        r10: 20,
        // skip r11
        r12: 22,
        r13: 23,
        r14: 24,
        r15: 25,
        ..cx.general
    }
);
assert_eq!(cx.trap_num, 0x100);
assert_eq!(cx.error_code, 0);
</pre></td>
<td>
6. 检查保存的寄存器<br>
<img src="fncallfiles/run_fncall/9.png"></img>

【注】：除rax之外，其余寄存器都相等
</td>
</tr>

</table>
