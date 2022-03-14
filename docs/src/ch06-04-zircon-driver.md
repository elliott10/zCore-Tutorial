
<!---
# Zircon Driver Development Kit
--->
# Zircon驱动开发工具（DDK）
---

* [设备模型（英文原文）](https://github.com/fuchsia-mirror/zircon/blob/3adf3875541d28ad944637f753f8e454fa91dceb/docs/ddk/device-model.md)
* [设备的操作（英文原文）](https://github.com/fuchsia-mirror/zircon/blob/3adf3875541d28ad944637f753f8e454fa91dceb/docs/ddk/device-ops.md)
* [驱动开发（英文原文）](https://github.com/fuchsia-mirror/zircon/blob/3adf3875541d28ad944637f753f8e454fa91dceb/docs/ddk/driver-development.md)
* [平台总线（英文原文）](https://github.com/fuchsia-mirror/zircon/blob/3adf3875541d28ad944637f753f8e454fa91dceb/docs/ddk/platform-bus.md)

---
# Zircon驱动开发

Zircon驱动程序是在设备主机进程的用户空间中动态加载的共享库。加载驱动程序的进程由Device Coordinator控制。有关详细信息，请参见[Device Model]（device-model.md）设备主机，Device Coordinator以及驱动程序和设备生命周期。

## 目录结构

Zircon驱动程序位于[system/dev](../../system/dev)下。它们根据实现的协议进行分组。驱动程序协议定义于[ddk/include/ddk/protodefs.h](../../system/ulib/ddk/include/ddk/protodefs.h)。例如，USB以太网驱动程序进入[system/dev/ethernet](../../system/dev/ethernet)而不是[system/dev/usb](../../system/dev/usb)，因为它实现了以太网协议。但是，实现USB协议栈的驱动程序位于[system/dev/usb](../../system/dev/usb)中因为他们实现USB协议。

在驱动程序的`rules.mk`中，`MODULE_TYPE`应该是'driver'。这将在`/boot/driver/`中安装驱动程序共享库。

如果您的驱动程序是在Zircon之外构建的，请将它们安装在`/system/driver/`中。Device Coordinator在这些目录中查找可加载的驱动程序。

## 声明一个驱动程序

驱动程序至少应包含驱动程序声明并实现`bind（）`驱动程序op。

在Device Coordinator成功找到设备的匹配驱动程序时，驱动程序被加载并绑定到设备。驱动通过bindings来声明它兼容的设备。以下绑定程序声明[AHCI driver](../../system/dev/block/ahci/ahci.c)：

```
ZIRCON_DRIVER_BEGIN（ahci，ahci_driver_ops，“Zircon”，“0.1”，4）
    BI_ABORT_IF（NE，BIND_PROTOCOL，ZX_PROTOCOL_PCI），
    BI_ABORT_IF（NE，BIND_PCI_CLASS，0x01），
    BI_ABORT_IF（NE，BIND_PCI_SUBCLASS，0x06），
    BI_MATCH_IF（EQ，BIND_PCI_INTERFACE，0x01），
ZIRCON_DRIVER_END（AHCI）
```

AHCI驱动程序在绑定程序中有4个指令。 “Zircon”是供应商id、“0.1”是驱动程序版本。它与`ZX_PROTOCOL_PCI`设备绑定PCI类1，子类6，接口1。

[PCI驱动程序](../../system/dev/bus/pci/kpci.c) 发布匹配具有以下属性的设备：
```
zx_device_prop_t device_props[] = {undefined
    {BIND_PROTOCOL, 0, ZX_PROTOCOL_PCI},
    {BIND_PCI_VID, 0, info.vendor_id},
    {BIND_PCI_DID, 0, info.device_id},
    {BIND_PCI_CLASS, 0, info.base_class},
    {BIND_PCI_SUBCLASS, 0, info.sub_class},
    {BIND_PCI_INTERFACE, 0, info.program_interface},
    {BIND_PCI_REVISION, 0, info.revision_id},
    {BIND_PCI_BDF_ADDR, 0, BIND_PCI_BDF_PACK(info.bus_id, info.dev_id,
                                             info.func_id)},
};
```
绑定变量和宏定义于[zircon/driver/binding.h](../../system/public/zircon/driver/binding.h)。如果要引入新的设备类，则可能需要在该文件中引入新的绑定变量。绑定变量是32位值。如果你的变量值需要大于32位的值，将它们分成多个32位变量。一个示例是ACPI HID值，长度为8个字符（64位）。它拆分为“BIND_ACPI_HID_0_3”和“BIND_ACPI_HID_4_7”。

绑定指令按顺序进行评估。分支指令`BI_GOTO（）`和`BI_GOTO_IF（）`允许你跳转到匹配标签，由`BI_LABEL（）`定义。

可以使用`BI_ABORT_IF_AUTOBIND`（通常作为第一条指令）防止默认的自动绑定行为。在这种情况下，驱动程序可以调用`ioctl_device_bind（）`来绑定到设备。


## 驱动程序绑定

驱动程序的`bind（）`函数在与设备匹配时被调用。通常，驱动程序将初始化设备所需的任何数据结构并在此功能中初始化硬件。它不应该执行任何耗时的任务或阻塞在这个函数中，因为它是调用的devhost的RPC线程，它将无法同时服务于其他请求。相反，它应该生成一个新线程来执行冗长的任务。

驱动程序不应该在`bind（）`中对硬件的状态做出任何假设，重置硬件或确保它处于已知状态。因为系统通过重新生成devhost从驱动程序崩溃恢复，此时调用`bind（）`，硬件可能是未知的的状态。

驱动程序需要通过调用`device_add（）`在`bind（）`中发布`zx_device_t`。这对于Device Coordinator追踪设备生命周期来说是必需的。如果驱动程序在`bind（）`中无法发布一个有功能的设备，例如，如果它正在一个线程中初始化整个设备，它应该发布一个不可见的设备，并且当初始化完成时使该设备可见。参见`DEVICE_ADD_INVISIBLE`和`device_make_visible（）`在[zircon/ddk/driver.h](../../system/ulib/ddk/include/ddk/driver.h)。

`bind（）`通常有四个结果：

1.驱动程序确定设备是否受支持，不需要执行任何繁重的操作，所以通过`device_add（）`发布一个新设备并返回`ZX_OK`。

2.即使绑定程序与设备匹配，驱动程序也可能不支持（可能是由于检查hw版本位或诸如此类）并返回错误。

3.在设备准备好之前，驱动程序需要进行进一步的初始化或确定它可以支持它，因此它发布了一个隐形设备并启动了一个线程继续工作，同时返回`ZX_OK`。那个线程最终会使设备可见，或者如果无法成功初始化设备，将其删除。

4.驱动程序代表一个总线或控制器，可能有0..n个孩子动态出现或消失。在这种情况下，它应该发布一个设备立即代表总线或控制器，然后动态发布子代（下游驱动程序将绑定到的子代）代表硬件总线。示例：AHCI/SATA，USB等。

添加设备并使系统可见后，它将使客户端进程可见，并且提供兼容驱动程序的绑定功能。

## 设备协议

驱动程序为设备提供一组设备操作和可选协议操作。
设备操作实现设备生命周期方法和到其他用户空间应用程序和服务调用的外部设备接口。
协议操作实现被其他驱动调用设备的ddk内部协议。

您可以在`device_add_args_t`中为设备传递一组协议操作。 如果设备支持多种协议，实现`get_protocol（）`设备op。 设备只能有一个协议ID。 协议ID与class交互，设备在devfs下发布。

设备协议头位于[ddk/protocol/](../../system/ulib/ddk/include/ddk/protocol)。 驱动程序之间传递的ops和任何数据结构应在此头中定义。

## 驱动程序操作

驱动程序通常为来自child驱动的客户请求提供服务或其他过程。它通过直接与使用硬件（例如，通过MMIO）通信或与父设备（例如，排队USB事务）通信来满足这些要求。

来自devhost之外的进程的外部客户端请求将通过以下方式完成设备操作:`message（）`，`read（）`，`write（）`和`ioctl（）`。来自child驱动的请求，通常在同一进程中，由设备与设备类对应的协议来完成。驱动程序到驱动程序的请求应该使用设备协议而不是设备操作。

设备可以通过调用`device_get_protocol（）`获得其父级支持的协议。

## 设备中断

设备中断通过中断对象实现，中断对象是一种内核对象类型。驱动程序通过一个设备协议方法来从其父设备请求设备中断句柄。返回的句柄将根据父驱动程序的定义，被绑定到适当设备中断。例如，PCI协议为PCI子节点实现`map_interrupt（）`。一个驱动程序应该生成一个线程来等待中断句柄。

内核将根据中断是否是边沿触发或电平触发，来自动处理屏蔽和取消屏蔽适当的中断。对于电平触发的硬件中断，[zx_interrupt_wait()](../syscalls/interrupt_wait.md)将在返回之前屏蔽中断，且当[zx_interrupt_wait()]下一次被调用时取消屏蔽。对于边沿触发中断，中断保持未屏蔽状态。

中断线程不应执行任何长时间运行的任务。对于驱动执行冗长的任务，使用工作线程。

你可以用[zx_interrupt_signal()](../syscalls/interrupt_signal.md)向中断句柄发信号，用** ZX_INTERRUPT_SLOT_USER **从[zx_interrupt_wait()]返回。这是在驱动程序清理期间关闭中断线程所必需的。

## FIDL消息

每个设备类的消息都在[FIDL](../../../docs/development/languages/fidl/README.md)语言中定义。每个设备实现零个或多个FIDL接口，多路复用每个客户端的通道。驱动有机会通过`message（）`钩子解释FIDL消息。

## Ioctl

不推荐使用Ioctls以支持消息。不应该实现新的ioctls。

每个设备类的Ioctls定义在[zircon/device/](../../system/public/zircon/device)。 Ioctls可以接受或返回句柄。 `IOCTL_KIND_ *`定义于[zircon/device/ioctl.h](../../system/public/zircon/device/ioctl.h)，用于ioctl声明，定义ioctl是接受还是返回多少句柄。驱动程序拥有传入的句柄，它们不再需要时应该关闭句柄，除非它返回`ZX_ERR_NOT_SUPPORTED`在这种情况下devhost RPC层将关闭句柄。

## 协议ops与FIDL消息

协议操作定义设备的DDK内部API。 FIDL消息定义了外部API。如果要被其他驱动调用函数，则定义协议op。驱动程序应该在其父级上调用协议op来使用那些功能。

## 隔离设备

使用`DEVICE_ADD_MUST_ISOLATE`添加的设备会产生新的代理devhost。该设备既存在于父devhost中，也存在于新devhost的根目录中。Devmgr尝试将<driver> .proxy.so加载到此代理devhost中。例如，PCI由libpci.so提供，因此devmgr会加载libpci.proxy.so。该驱动程序在创建代理设备时在`create（）`中提供了一个通道（在新的devhost中运行的“下半部分”）。代理设备当需要与上半部分通信时应该缓存通道（例如，如果它需要在父设备上调用API）。

当该通道在下半部分被写入时，在上半部分调用`rxrpc（）`。此通道没有通用的线路协议。为例如，请参阅[PCI driver](../../system/dev/bus/pci)。

注意：这是各种总线设备使用的机制，而不是一般驱动应该担心的某种东西。 （如果你认为需要用这个，请ping swetland）

## Logging

[ddk/debug.h](../../system/ulib/ddk/include/ddk/debug.h)定义了`zxlogf（<log_level>，...）`宏。如果设备可用日志消息将通过网络和串行端口上的debuglog打印到系统中。默认情况下，始终打印“ERROR”和“INFO”。您可以通过传递启动参数，为一个驱动控制日志级别`driver.<driver_name>.log=+<level>,-<level>`。例如，`driver.sdhci.log=-info,+trace,+spew`启用`TRACE`和`SPEW`日志和禁用sdhci驱动程序的`INFO`日志。

以“L”（“LERROR”，“LINFO”等）为前缀的日志级别不会被发送网络，对网络日志记录非常有用。

## 驱动程序测试

`ZX_PROTOCOL_TEST`提供了一种通过在模拟环境中运行驱动程序来测试驱动程序的机制。写一个绑定到`ZX_PROTOCOL_TEST`设备的驱动程序。此驱动程序应发布一个在测试中可以绑定驱动程序的设备，并且它应该实现协议功能和被测驱动程序的正常操作流程。这个辅助驱动程序应该是在绑定中使用`BI_ABORT_IF_AUTOBIND`声明。

测试工具在`/dev/test/test`上调用`fuchsia.device.test.RootDevice.CreateDevice()`，它将创建一个`ZX_PROTOCOL_TEST`设备并返回它的路径。然后它在新创建的设备上使用辅助驱动程序调用`ioctl_device_bind（）`。这种方法通常在中层协议驱动程序工作得很好。也可以使用相同的方法模拟真实硬件，但它可能没那么有用。

定义在
[ddk/protocol/test.h](../../system/ulib/ddk/include/ddk/protocol/test.h)的函数是用于测试驱动程序的一部分运行的库。有关示例，请参阅[system/ulib/ddk/test](../../system/ulib/ddk/test)。这些测试工具测试在[system/utest/driver-tests/main.c](../../system/utest/driver-tests/main.c)

## 驱动程序权限

虽然驱动程序在用户空间进程中运行，但它们具有比正常程序更受限制的集合权利。不允许驱动访问文件系统，包括devfs。这意味着驱动无法与任意设备互动。如果您的驱动程序需要这样做，请考虑编写服务代替。例如，虚拟控制台由[virtcon](../../system/core/virtcon)服务。

特权操作，例如`zx_vmo_create_contiguous（）`和[zx_interrupt_create](../syscalls/interrupt_create.md)需要一个根资源句柄。此句柄不适用于系统驱动程序以外的驱动程序([ACPI](../../system/dev/bus/acpi) on x86 systems and
[platform](../../system/dev/bus/platform) on ARM systems)。设备应该请求其父级为其执行此类操作。联系父驱动程序作者如果其协议不解决此场景。

同样，不允许驱动程序请求任意MMIO范围，中断或GPIOs。PCI和platform等总线驱动程序只返回与子设备关联的资源。
