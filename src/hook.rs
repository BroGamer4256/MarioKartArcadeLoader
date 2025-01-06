pub unsafe fn aslr(address: usize) -> *mut () {
	let module = windows::Win32::System::LibraryLoader::GetModuleHandleA(windows::core::PCSTR(
		std::ptr::null() as _,
	))
	.unwrap()
	.0 as usize;
	let address = module + address - 0x400000usize;
	address as *mut ()
}

pub unsafe fn hook(address: *mut (), func: *const ()) -> *const () {
	let Ok(hook) = retour::RawDetour::new(address, func) else {
		return std::ptr::null();
	};
	let Ok(_) = hook.enable() else {
		return std::ptr::null();
	};
	let trampoline = hook.trampoline() as *const ();
	std::mem::forget(hook);
	trampoline
}

pub unsafe fn write_memory(address: *mut (), data: &[u8]) {
	region::protect(address, data.len(), region::Protection::READ_WRITE_EXECUTE).unwrap();
	std::ptr::copy_nonoverlapping(data.as_ptr(), address as *mut u8, data.len());
}
