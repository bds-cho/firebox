use crate::dto::{VmDetail, VmSummary};

pub fn print_vm_list(vms: &[VmSummary]) {
    if vms.is_empty() {
        println!("No VMs found.");
        return;
    }
    println!("{:<38} {}", "ID", "STATUS");
    for vm in vms {
        println!("{:<38} {}", vm.id, vm.status);
    }
}

pub fn print_vm_detail(vm: &VmDetail) {
    println!("ID:         {}", vm.id);
    println!("Status:     {}", vm.status);
    println!("vCPUs:      {}", vm.vcpus);
    println!("Memory:     {} MB", vm.memory_mb);
    println!("Kernel:     {}", vm.kernel);
    println!("Rootfs:     {}", vm.rootfs);
    if let Some(pid) = vm.pid {
        println!("PID:        {}", pid);
    }
}
