# zCore 整体结构和设计模式

首先，从 [Rust语言操作系统的设计与实现,王润基本科毕设论文,2019](https://github.com/rcore-os/zCore/wiki/files/wrj-thesis.pdf) <br>
和 [zCore操作系统内核的设计与实现,潘庆霖本科毕设论文,2020](https://github.com/rcore-os/zCore/wiki/files/pql-thesis.pdf) <br>
可以了解到从 rCore 的设计到 zCore 的设计过程的全貌。

## zCore 的整体结构

[zCore](https://github.com/rcore-os/zCore) 的整体结构/项目设计图如下：

![img](zcore-intro/structure.svg)

在实际的架构规划中，zCore项目在Zircon的设计基础上进行了更深入的思考。<br>
Zircon的内核对象这一设计思路，引导我们对整个zCore项目的构建进行了重新思考。并基于以下几点考虑，来设计整个zCore项目的框架。<br>
zCore的设计主要有两个出发点：

- 内核对象的封装：将内核对象代码封装为一个库，保证可重用。内核对象在C++中实现为类，在Rust中实现为结构。这部分代码应当被封装为一个库，使其具有易被复用的特点。
- 硬件接口的设计：使硬件与内核对象的设计相对独立，只向上提供统一、抽象的API接口。内核对象的实现需要操作硬件，但应当可以独立于硬件，这样我们能够更方便地支持libos的运行。可以通过设计一套硬件接口，向下封装具体的硬件操作代码，向上提供统一、抽象的API接口，这种做法同时有利于我们将直接操作硬件的Unsafe Rust代码进行单独封装，方便Rust Safety的检查与保证。

项目设计从上到下，上层更远离硬件，下层更接近硬件。<br>
各个模块在具体实现中都被封装为对应的库，被上一层所调用。<br>

1. zCore 设计的顶层是上层操作系统，比如 zCore、rCore、Zircon LibOS 和 Linux LibOS。在项目架构中，各版本的操作系统有部分公用代码。与 zCore 微内核设计实现相关的部分则主要是图中左侧蓝色线部分。

2. 第二层，是 ELF 程序加载层（ELF Program Loader），分别包括 zircon 和 linux 的 loader，其中封装了初始化内核对象、部分硬件相关的初始化、设定系统调用接口、运行首个用户态程序等逻辑，并形成一个库函数。zCore 在顶层通过调用 loader 库中的初始化逻辑，进入第一个用户态程序执行。

3. 第三层，是系统调用实现层（Syscall Implementation），包括 zircon-syscall 和 linux-syscall，这一层将所有的系统调用处理例程封装为一个系统调用库，供上方操作系统使用。

4. 第四层，利用硬件抽象层提供的虚拟硬件 API 进行内核对象（Kernel Objects）的实现，并且基于实现的各类内核对象，实现第三层各个系统调用接口所需要的具体处理例程。

5. 第五层，是硬件抽象层（HAL，Hardware Abstraction Layer），这里对应的是 kernel-hal 模块。kernel-hal 将向上提供所有操作硬件需要的接口，从而使得硬件环境对上层操作系统透明化。

6. 第六层，是对直接操作硬件的代码进行一层封装，对应模块为 kernel-hal-bare 和 kernel-hal-unix（也称libos)。kernel-hal 系列库仅仅负责接口定义，即将底层硬件/宿主操作系统的操作翻译为上层操作系统可以使用的形式。在这里，kernel-hal/bare 负责翻译裸机的硬件功能，其中模块drivers实现了众多的硬件驱动程序，而 kernel-hal/libos 则负责翻译类 Unix 系统的系统调用。

7. 最底层是底层运行环境，包括 Bare Metal（裸机），Linux / macOS 操作系统。Bare Metal可以认为是硬件架构上的寄存器等硬件接口。

## zCore 内核组件

zCore 内核运行时组件层次概况如下：

<img src="zcore-intro/image-20200805123801306.png" width="60%" />

### zCore启动
在zCore启动过程中，会初始化物理页帧分配器、堆分配器、线程调度器等各个组成部分。并委托 zircon-­loader 进行内核对象的初始化创建过程，然后进入用户态的启动过程开始执行。每当用户态触发系统调用进入内核态，系统调用处理例程将会通过已实现的内核对象的功能来对服务请求进行处理；而对应的内核对象的内部实现所需要的各种底层操作，则是通过 HAL 层接口由各个内核组件负责提供。

### async协程
协程是Rust中对高并发特性的一个有力支持，Rust通过编译器检查结合运行时内存分配，实现了一套无栈协程机制。在语法上，Rust给出了async/await关键字和Future Trait接口，开发者可以通过实现对应的接口，来封装底层Future对象；<br>
在zCore 开发中，我们使用no_std 情况下的async 相关语法，来将用户线程封装为内核态中的async 协程，真实目的是希望能够借助Rust 强大的编译期检查，将传统的线程用内核态的协程来实现，不仅减少了内存占用，同时也创造更好的内核开发环境，真正在内核的开发中体现Rust 强大的并发特性。实际应用中，我们除了能够直接借助的async 语法支持，还需要自行提供不依赖于std的协程调度器，这一开销相比async 语法机制为zCore 带来的好处来说，完全在可接受的范围内。

### VDSO
VDSO（Virtual dynamic shared object）是一个映射到用户空间的 so 文件，可以在不陷入内核的情况下执行一些简单的系统调用。在设计中，所有中断都需要经过 VDSO 拦截进行处理，因此重写 VDSO 便可以实现自定义的对下层系统调用（syscall）的支持。Executor 是 zCore 中基于 Rust 的 `async` 机制的协程调度器。<br>
VDSO是由内核提供、内核负责映射的动态链接库，以函数接口形式提供系统调用接口。原始的VDSO中将会最终使用syscall指令从用户态进入内核态。但在libos环境下，内核和用户程序都运行在用户态，因此需要将syscall指令修改为函数调用，也就是将sysall指令修改为call 指令。在libos内核初始化环节中，将VDSO中的syscall指令修改为call指令，并指定跳转的目标地址，重定向到内核中处理syscall的特定函数，从而实现模拟系统调用的效果。

### 地址空间
在libos中，使用mmap来模拟页表，所有进程共用一个64位地址空间。因此，从地址空间范围这一角度来说，运行在libos 上的用户程序所在的用户进程地址空间无法像Zircon要求的一样大。对于这一点，我们在为每一个用户进程设置地址空间时，手动进行分配，规定每一个用户进程地址空间的大小为0x100_0000_0000，从0x2_0000_0000开始依次排布。0x0开始至0x2_0000_0000规定为libos内核所在地址空间，不用于mmap。下图给出了libos在运行时若干个用户进程的地址空间分布。<br>
<img src="img/zcore_libos1.png" width="60%" />

与libos兼容，zCore对于用户进程的地址空间划分也遵循同样的设计，但在裸机环境下，一定程度上摆脱了限制，能够将不同用户地址空间分隔在不同的页表中。如图所示，zCore中将三个用户进程的地址空间在不同的页表中映射，但是为了兼容libos的运行，每一个用户进程地址空间中用户程序能够真正访问到的部分都仅有0x100_0000_0000大小。<br>
<img src="img/zcore_libos2.png" width="60%" />

### HAL层接口
在HAL接口层的设计上，还借助了 Rust 的能够指定函数链接过程的特性。即，在 kernel-­hal 中规定了所有可供 zircon­-object 库及 zircon-­syscall 库调用的虚拟硬件接口，以函数 API 的形式给出，但是内部均为未实现状态，并设置函数为弱引用链接状态。在 kernel­-hal-­bare 中才给出裸机环境下的硬件接口具体实现，编译 zCore 项目时、链接的过程中将会替换/覆盖 kernel-­hal 中未实现的同名接口，从而达到能够在编译时灵活选择 HAL 层的效果。<br>
对内核对象层而言，所依赖的硬件环境不再是真实硬件环境中能够看到的物理内存、CPU、MMU等，而是HAL暴露给上层的一整套接口。这一点从设计上来说，是zCore与Zircon存在差异的一点。Zircon将x86_64和ARM64两个硬件架构进行底层封装，但是没有给出一套统一的硬件API 供上层的内核对象直接使用，在部分内核对象的实现中，仍然需要通过宏等手段对代码进行条件编译，从而支持同时面向两套硬件架构进行开发。而在zCore的内核对象层实现中，可以完全不考虑底层硬件接口的实现，使一套内核对象的模块代码可以同时在zCore和libos上运行，之后如果zCore进一步支持ARM64架构，只需要新增一套HAL的实现，无需修改上层代码。<br>
HAL层的部分接口设计，覆盖线程调度与内存管理两方面。<br>
zCore HAL层部分接口概况<br>
<img src="img/zcore_hal_interface.png" width="60%" />

### 代码结构
zCore在实际代码层面将代码进行了目录结构的调整，为了便于将具体设计与代码相结合，在此给出更具体的目录结构说明。在图中，绿字直接对应项目中的具体目录名称。
在目录结构的设计上，充分考虑Rust在模块化设计上的优势和链接过程中体现的灵活性，将可以被独立出来的模块作为子目录，共同组成整个cargo工程目录。在HAL接口层的设计上，我们借助Rust的能够指定函数链接过程的特性。另外HAL层的接口设计文档也可以借助Rust的自动生成文档功能，自然地在kernel-hal中给出。<br>
zCore代码统计<br>
<img src="img/zcore_arch_lines.png" width="70%" />

### Unsafe Rust在zCore代码中的分布
在zCore中，unsafe代码中的绝大多数都被封装在了HAL层中，被认为是关键代码。但由于在内核对象层和系统调用层使用了核心库中一些与内存操作相关的底层函数，在使用这些函数时需要添加unsafe标记。HAL层及以下的部分代码中，使用unsafe语法是必不可少的，因此在这里我们不对HAL层及以下的代码进行safety 分析；

在表格中，我们对HAL层之上的unsafe代码进行统计和简单的safety说明，对相关的这些safety说明，将在下一小节给出描述。<br>
zCore HAL层之上unsafe分布情况<br>
|对应模块|使用原因|使用次数|
|-|-|-|
|zircon-object|强制裸指针转换<br> Arc::get_mut_unchecked<br> transmute/transmute_copy<br> 读/写union，core::slice::from_raw_parts|14|
|zirconsyscall|读取随机数|1|
1. 裸指针转换在这里主要指将usize直接转换为具有可变引用权限的指针；
2. Arc指针是Rust中的线程安全引用计数指针，每一个Arc指针可以认为是目标结构体实例的一个不可变引用，正常情况下，Arc指针对目标结构体实例持有不可变引用。zCore中对某些的Arc指针进行了获取可变引用的操作；
3. transmute/transmute_copy 是Rust 核心库中提供的两个对内存进行操作的底层函数，主要用于内存布局转换和内存直接复制；
4. Rust中的Union相比C/C++ 中的在灵活性上更受限制。在Rust中，关于Union的全部操作被认为是unsafe的。
5. from_raw_parts是Rust核心库提供的基于常数指针的类型转换函数，支持将一个指向特定结构体的常数指针转为一个指向同类型结构体数组的不可变引用。
6. 用于读取随机数的相关系统调用的实现例程中，我们直接使用了Rust核心库支持的_rdrand32_step函数，该函数本身被标记为unsafe。




