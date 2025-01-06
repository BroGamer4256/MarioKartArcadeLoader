#![feature(let_chains)]
#![allow(static_mut_refs)]
use libc::*;
use std::mem::transmute;

pub mod hook;
pub mod poll;

pub static mut CONFIG: Config = default_config();
pub static mut KEYCONFIG: KeyConfig = default_keyconfig();

#[derive(serde::Deserialize)]
pub struct Config {
	fullscreen: bool,
	dongle: String,
	deadzone: f32,
	width: u32,
	height: u32,
}

pub struct KeyConfig {
	test: poll::KeyBindings,
	service: poll::KeyBindings,
	test_up: poll::KeyBindings,
	test_down: poll::KeyBindings,
	test_enter: poll::KeyBindings,

	brake: poll::KeyBindings,
	gas: poll::KeyBindings,
	item: poll::KeyBindings,
	mario: poll::KeyBindings,
	wheel_left: poll::KeyBindings,
	wheel_right: poll::KeyBindings,
}

const fn default_config() -> Config {
	Config {
		fullscreen: false,
		dongle: String::new(),
		deadzone: 0.01,
		width: 1920,
		height: 1080,
	}
}

const fn default_keyconfig() -> KeyConfig {
	KeyConfig {
		test: poll::KeyBindings { keys: vec![] },
		service: poll::KeyBindings { keys: vec![] },
		test_up: poll::KeyBindings { keys: vec![] },
		test_down: poll::KeyBindings { keys: vec![] },
		test_enter: poll::KeyBindings { keys: vec![] },

		brake: poll::KeyBindings { keys: vec![] },
		gas: poll::KeyBindings { keys: vec![] },
		item: poll::KeyBindings { keys: vec![] },
		mario: poll::KeyBindings { keys: vec![] },
		wheel_left: poll::KeyBindings { keys: vec![] },
		wheel_right: poll::KeyBindings { keys: vec![] },
	}
}

static mut ORIGINAL_DINPUT8: Option<libloading::Library> = None;
static mut ORIGINAL_DINPUT8_CREATE: Option<
	libloading::Symbol<
		unsafe extern "stdcall" fn(
			hinst: *const c_void,
			dwVersion: i32,
			riidltf: *const c_void,
			ppvOut: *const *const c_void,
			punkOuter: *const c_void,
		) -> i32,
	>,
> = None;
#[no_mangle]
#[allow(non_snake_case)]
unsafe extern "stdcall" fn DirectInput8Create(
	hinst: *const c_void,
	dwVersion: i32,
	riidltf: *const c_void,
	ppvOut: *const *const c_void,
	punkOuter: *const c_void,
) -> i32 {
	ORIGINAL_DINPUT8_CREATE.as_ref().unwrap()(hinst, dwVersion, riidltf, ppvOut, punkOuter)
}

unsafe extern "fastcall" fn adachi() -> c_int {
	1
}

pub static mut HASP_BUFFER: [u8; 0xD40] = [0u8; 0xD40];

unsafe extern "stdcall" fn hasp_decrypt(_id: c_int, _buffer: *mut u8, _size: c_int) -> c_int {
	0
}

unsafe extern "stdcall" fn hasp_login(_id: c_int, _key: *const c_char, _a3: *const c_int) -> c_int {
	0
}

unsafe extern "stdcall" fn hasp_logout(_id: c_int) -> c_int {
	0
}

unsafe extern "stdcall" fn hasp_read(
	_id: c_int,
	_flags: c_int,
	offset: c_int,
	length: c_int,
	buffer: *mut u8,
) -> c_int {
	std::ptr::copy_nonoverlapping(
		HASP_BUFFER.as_ptr().offset(offset as isize),
		buffer,
		length as usize,
	);
	0
}

unsafe extern "stdcall" fn hasp_write(
	_id: c_int,
	_flags: c_int,
	offset: c_int,
	length: c_int,
	data: *const u8,
) -> c_int {
	std::ptr::copy_nonoverlapping(
		data,
		HASP_BUFFER.as_mut_ptr().offset(offset as isize),
		length as usize,
	);

	let mut crc = 0u8;
	for byte in HASP_BUFFER.iter().take(0xD3E) {
		crc += byte;
	}
	HASP_BUFFER[0xD3E] = crc;
	HASP_BUFFER[0xD3F] = crc ^ 0xFF;

	0
}

unsafe extern "fastcall" fn dongle_check(_a1: *const c_void, a2: *mut *mut u8) -> c_int {
	a2.byte_offset(0xD8)
		.write(HASP_BUFFER.as_mut_ptr().offset(0xD00));
	0
}

unsafe extern "fastcall" fn input_loop(a1: *mut u8) {
	let handle = hook::aslr(0xf8bec4) as *const *const c_void;
	let handle = handle.read();
	let mut state = poll::PollState::new(handle, CONFIG.deadzone).unwrap();

	let input = a1.byte_offset(0x14);
	loop {
		std::thread::sleep(std::time::Duration::from_millis(8));
		state.update();

		if state.is_tapped(&KEYCONFIG.test) {
			input
				.byte_offset(0x187)
				.write(!input.byte_offset(0x187).read());
		}

		input
			.byte_offset(0x18C)
			.write(state.is_tapped(&KEYCONFIG.service) as u8);
		input
			.byte_offset(0x18D)
			.write(state.is_tapped(&KEYCONFIG.test_up) as u8);
		input
			.byte_offset(0x18E)
			.write(state.is_tapped(&KEYCONFIG.test_down) as u8);
		input
			.byte_offset(0x191)
			.write(state.is_tapped(&KEYCONFIG.test_enter) as u8);
		input
			.byte_offset(0x193)
			.write((state.is_down(&KEYCONFIG.brake) > 0.5) as u8);
		input
			.byte_offset(0x195)
			.write((state.is_down(&KEYCONFIG.item) > 0.5) as u8);
		input
			.byte_offset(0x199)
			.write((state.is_down(&KEYCONFIG.mario) > 0.5) as u8);

		let wheel_left = state.is_down(&KEYCONFIG.wheel_left);
		let wheel_right = state.is_down(&KEYCONFIG.wheel_right);

		input.byte_offset(0x28D).write(
			(i8::MAX as f32 - (wheel_left * i8::MAX as f32) + (wheel_right * i8::MAX as f32)) as u8,
		);
		input
			.byte_offset(0x28F)
			.write((state.is_down(&KEYCONFIG.gas) * u8::MAX as f32) as u8);
	}
}

#[repr(C)]
union CxxStringUnion {
	data: [c_char; 16],
	ptr: *mut c_char,
}

#[repr(C)]
struct CxxString {
	union: CxxStringUnion,
	length: size_t,
	capacity: size_t,
}

impl CxxString {
	unsafe fn c_string(&mut self) -> *mut c_char {
		if self.capacity > 0x0F {
			self.union.ptr
		} else {
			&mut self.union.data as *mut c_char
		}
	}
}

static mut ORIGINAL_FILEOPEN: Option<
	unsafe extern "fastcall" fn(
		file: *mut CxxString,
		folder: *mut CxxString,
		a3: *mut c_int,
		flags: *const c_char,
		a5: c_int,
	) -> c_int,
> = None;
unsafe extern "fastcall" fn fileopen(
	file: *mut CxxString,
	folder: *mut CxxString,
	a3: *mut c_int,
	flags: *const c_char,
	a5: c_int,
) -> c_int {
	if let Some(folder) = folder.as_mut() {
		folder.c_string().byte_offset(0).write(b'.' as c_char);
		folder.c_string().byte_offset(1).write(b'/' as c_char);
		folder.c_string().byte_offset(2).write(b'\0' as c_char);
		folder.length = 2;
	}

	ORIGINAL_FILEOPEN.unwrap()(file, folder, a3, flags, a5)
}

static mut ORIGINAL_FILESAVE: Option<
	unsafe extern "fastcall" fn(
		file: *mut CxxString,
		a2: *mut c_int,
		a3: *mut c_int,
		folder: *mut CxxString,
		a5: *mut c_int,
		a6: *mut c_int,
		a7: c_int,
		size: c_int,
		a9: c_int,
	),
> = None;
unsafe extern "fastcall" fn filesave(
	file: *mut CxxString,
	a2: *mut c_int,
	a3: *mut c_int,
	folder: *mut CxxString,
	a5: *mut c_int,
	a6: *mut c_int,
	a7: c_int,
	size: c_int,
	a9: c_int,
) {
	if let Some(folder) = folder.as_mut() {
		folder.c_string().byte_offset(0).write(b'.' as c_char);
		folder.c_string().byte_offset(1).write(b'/' as c_char);
		folder.c_string().byte_offset(2).write(b'\0' as c_char);
		folder.length = 2;
	}

	ORIGINAL_FILESAVE.unwrap()(file, a2, a3, folder, a5, a6, a7, size, a9)
}

static mut ORIGINAL_SET_WINDOW_LONG_W: Option<
	unsafe extern "stdcall" fn(hwnd: *const c_void, index: c_int, new_long: c_long) -> c_long,
> = None;
unsafe extern "stdcall" fn set_window_long_w(
	hwnd: *const c_void,
	index: c_int,
	new_long: c_long,
) -> c_long {
	if index == -16 {
		ORIGINAL_SET_WINDOW_LONG_W.unwrap()(
			hwnd,
			index,
			new_long | windows::Win32::UI::WindowsAndMessaging::WS_VISIBLE.0 as c_long,
		)
	} else if index == -20 {
		ORIGINAL_SET_WINDOW_LONG_W.unwrap()(
			hwnd,
			index,
			new_long & !windows::Win32::UI::WindowsAndMessaging::WS_EX_TOPMOST.0 as c_long,
		)
	} else {
		ORIGINAL_SET_WINDOW_LONG_W.unwrap()(hwnd, index, new_long)
	}
}

static mut ORIGINAL_ADJUST_WINDOW_RECT: Option<
	unsafe extern "stdcall" fn(rect: *mut c_long, style: c_int, menu: c_int) -> c_int,
> = None;
unsafe extern "stdcall" fn adjust_window_rect(
	rect: *mut c_long,
	style: c_int,
	menu: c_int,
) -> c_int {
	rect.offset(0).write(0);
	rect.offset(1).write(0);
	rect.offset(2).write(CONFIG.width as c_long);
	rect.offset(3).write(CONFIG.height as c_long);
	ORIGINAL_ADJUST_WINDOW_RECT.unwrap()(rect, style, menu)
}

static mut ORIGINAL_SET_WINDOW_POS: Option<
	unsafe extern "stdcall" fn(
		hwnd: *const c_void,
		hwnd_after: *const c_void,
		x: c_int,
		y: c_int,
		cx: c_int,
		cy: c_int,
		flags: c_uint,
	) -> c_int,
> = None;
unsafe extern "stdcall" fn set_window_pos(
	hwnd: *const c_void,
	hwnd_after: *const c_void,
	_x: c_int,
	_y: c_int,
	_cx: c_int,
	_cy: c_int,
	flags: c_uint,
) -> c_int {
	ORIGINAL_SET_WINDOW_POS.unwrap()(
		hwnd,
		hwnd_after,
		0,
		0,
		CONFIG.width as c_int,
		CONFIG.height as c_int,
		flags,
	)
}

unsafe extern "stdcall" fn set_window_placement(_: *const c_void, _: *const c_void) -> c_int {
	1
}

static mut ORIGINAL_CREATE_WINDOW: Option<
	unsafe extern "stdcall" fn(
		hwnd: *const c_void,
		class_name: *const u16,
		window_name: *const u16,
		style: c_uint,
		_x: c_int,
		_y: c_int,
		_width: c_uint,
		_height: c_uint,
		parent: *const c_void,
		menu: *const c_void,
		instance: *const c_void,
		param: *const c_void,
	) -> c_int,
> = None;
unsafe extern "stdcall" fn create_window(
	hwnd: *const c_void,
	class_name: *const u16,
	window_name: *const u16,
	style: c_uint,
	x: c_int,
	y: c_int,
	width: c_uint,
	height: c_uint,
	parent: *const c_void,
	menu: *const c_void,
	instance: *const c_void,
	param: *const c_void,
) -> c_int {
	if x == -1 && y == -1 {
		return ORIGINAL_CREATE_WINDOW.unwrap()(
			hwnd,
			class_name,
			window_name,
			style | windows::Win32::UI::WindowsAndMessaging::WS_VISIBLE.0,
			0,
			0,
			CONFIG.width,
			CONFIG.height,
			parent,
			menu,
			instance,
			param,
		);
	}

	ORIGINAL_CREATE_WINDOW.unwrap()(
		hwnd,
		class_name,
		window_name,
		style,
		x,
		y,
		width,
		height,
		parent,
		menu,
		instance,
		param,
	)
}

unsafe extern "stdcall" fn change_display_settings(
	_: *const c_char,
	_: *const c_void,
	_: *const c_void,
	_: c_int,
	_: *const c_void,
) -> c_long {
	0
}

unsafe extern "stdcall" fn show_cursor(_: c_int) -> c_int {
	-1
}

unsafe extern "stdcall" fn post_quit_message(exit_code: c_int) {
	exit(exit_code);
}

#[ctor::ctor]
unsafe fn init() {
	if let Ok(toml) = std::fs::read_to_string("config.toml") {
		if let Ok(toml) = toml::from_str(&toml) {
			CONFIG = toml;
		}
	}

	hook::hook(hook::aslr(0x85fb00), hasp_decrypt as *const ());
	hook::hook(hook::aslr(0x85f960), hasp_login as *const ());
	hook::hook(hook::aslr(0x85f9d0), hasp_logout as *const ());
	hook::hook(hook::aslr(0x860440), hasp_read as *const ());
	hook::hook(hook::aslr(0x8604f0), hasp_write as *const ());
	hook::hook(hook::aslr(0x5ec0f0), dongle_check as *const ());
	hook::hook(hook::aslr(0x5dd530), input_loop as *const ());
	hook::hook(hook::aslr(0x83ccd0), adachi as *const ());
	hook::hook(hook::aslr(0x7e4430), adachi as *const ());
	hook::hook(hook::aslr(0x7e46f0), adachi as *const ());
	hook::hook(hook::aslr(0x7e4410), adachi as *const ());
	hook::hook(hook::aslr(0x5dd9f0), adachi as *const ());

	ORIGINAL_FILEOPEN = Some(transmute(hook::hook(
		hook::aslr(0x7e0e70),
		fileopen as *const (),
	)));
	ORIGINAL_FILESAVE = Some(transmute(hook::hook(
		hook::aslr(0x7e35c0),
		filesave as *const (),
	)));

	hook::write_memory(hook::aslr(0x5e86e1), &[0xB0, 0x01, 0x90, 0x90]);

	hook::write_memory(hook::aslr(0x7d05ef), &[0x00]);
	hook::write_memory(hook::aslr(0x7d2274), &[0x00]);

	hook::write_memory(hook::aslr(0x4200ab), &CONFIG.width.to_le_bytes());
	hook::write_memory(hook::aslr(0x4200b2), &CONFIG.height.to_le_bytes());
	hook::write_memory(hook::aslr(0x404752), &CONFIG.width.to_le_bytes());
	hook::write_memory(hook::aslr(0x404759), &CONFIG.height.to_le_bytes());
	hook::write_memory(hook::aslr(0x406dbb), &CONFIG.height.to_le_bytes());
	hook::write_memory(hook::aslr(0x406dc0), &CONFIG.width.to_le_bytes());
	hook::write_memory(hook::aslr(0x406e12), &CONFIG.height.to_le_bytes());
	hook::write_memory(hook::aslr(0x406e17), &CONFIG.width.to_le_bytes());
	hook::write_memory(hook::aslr(0x406e2f), &CONFIG.height.to_le_bytes());
	hook::write_memory(hook::aslr(0x406e34), &CONFIG.width.to_le_bytes());
	hook::write_memory(hook::aslr(0x406dd8), &CONFIG.height.to_le_bytes());
	hook::write_memory(hook::aslr(0x406ddd), &CONFIG.width.to_le_bytes());
	hook::write_memory(hook::aslr(0x406df5), &CONFIG.height.to_le_bytes());
	hook::write_memory(hook::aslr(0x406dfa), &CONFIG.width.to_le_bytes());
	hook::write_memory(hook::aslr(0x6f205e), &CONFIG.width.to_le_bytes());
	hook::write_memory(
		hook::aslr(0x6f2066),
		&(CONFIG.height - (CONFIG.height / 36)).to_le_bytes(),
	);
	hook::write_memory(hook::aslr(0x6f20b5), &CONFIG.width.to_le_bytes());
	hook::write_memory(
		hook::aslr(0x6f20bd),
		&(CONFIG.height - (CONFIG.height / 36)).to_le_bytes(),
	);

	let library_handle = unsafe {
		windows::Win32::System::LibraryLoader::LoadLibraryA(windows::core::PCSTR(
			b"user32.dll\0".as_ptr() as _,
		))
	}
	.unwrap();

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"SetWindowLongW\0".as_ptr() as _),
		)
		.unwrap()
	};
	ORIGINAL_SET_WINDOW_LONG_W = Some(transmute(hook::hook(
		address as _,
		set_window_long_w as *const (),
	)));

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"AdjustWindowRect\0".as_ptr() as _),
		)
		.unwrap()
	};
	ORIGINAL_ADJUST_WINDOW_RECT = Some(transmute(hook::hook(
		address as _,
		adjust_window_rect as *const (),
	)));

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"SetWindowPos\0".as_ptr() as _),
		)
		.unwrap()
	};
	ORIGINAL_SET_WINDOW_POS = Some(transmute(hook::hook(
		address as _,
		set_window_pos as *const (),
	)));

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"SetWindowPlacement\0".as_ptr() as _),
		)
		.unwrap()
	};
	hook::hook(address as _, set_window_placement as *const ());

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"ChangeDisplaySettingsExW\0".as_ptr() as _),
		)
		.unwrap()
	};
	hook::hook(address as _, change_display_settings as *const ());

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"ShowCursor\0".as_ptr() as _),
		)
		.unwrap()
	};
	hook::hook(address as _, show_cursor as *const ());

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"SetCursor\0".as_ptr() as _),
		)
		.unwrap()
	};
	hook::hook(address as _, show_cursor as *const ());

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"CreateWindowExW\0".as_ptr() as _),
		)
		.unwrap()
	};
	ORIGINAL_CREATE_WINDOW = Some(transmute(hook::hook(
		address as _,
		create_window as *const (),
	)));

	let address = unsafe {
		windows::Win32::System::LibraryLoader::GetProcAddress(
			library_handle,
			windows::core::PCSTR(b"PostQuitMessage\0".as_ptr() as _),
		)
		.unwrap()
	};
	hook::hook(address as _, post_quit_message as *const ());

	let dongle = &CONFIG.dongle;
	std::ptr::copy_nonoverlapping(
		dongle.as_ptr(),
		HASP_BUFFER.as_mut_ptr().offset(0xD00),
		dongle.len(),
	);

	let mut crc = 0u8;
	for byte in HASP_BUFFER.iter().take(0xD3E) {
		crc += byte;
	}
	HASP_BUFFER[0xD3E] = crc;
	HASP_BUFFER[0xD3F] = crc ^ 0xFF;

	ORIGINAL_DINPUT8 = libloading::Library::new("C:/windows/system32/dinput8.dll").ok();
	ORIGINAL_DINPUT8_CREATE = ORIGINAL_DINPUT8
		.as_ref()
		.unwrap()
		.get(b"DirectInput8Create")
		.ok();

	// Really what I should do is implement a custom serde::Deserialize for KeyBindings
	// but serdes documentation is really confusing when it comes to this
	#[derive(serde::Deserialize)]
	#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
	struct KeyConfigTemp {
		test: Vec<String>,
		service: Vec<String>,
		test_up: Vec<String>,
		test_down: Vec<String>,
		test_enter: Vec<String>,

		brake: Vec<String>,
		gas: Vec<String>,
		item: Vec<String>,
		mario: Vec<String>,
		wheel_left: Vec<String>,
		wheel_right: Vec<String>,
	}

	let toml = std::fs::read_to_string("keyconfig.toml").unwrap();
	let keyconfig: KeyConfigTemp = toml::from_str(&toml).unwrap();
	KEYCONFIG = KeyConfig {
		test: poll::parse_keybinding(keyconfig.test),
		service: poll::parse_keybinding(keyconfig.service),
		test_up: poll::parse_keybinding(keyconfig.test_up),
		test_down: poll::parse_keybinding(keyconfig.test_down),
		test_enter: poll::parse_keybinding(keyconfig.test_enter),

		brake: poll::parse_keybinding(keyconfig.brake),
		gas: poll::parse_keybinding(keyconfig.gas),
		item: poll::parse_keybinding(keyconfig.item),
		mario: poll::parse_keybinding(keyconfig.mario),
		wheel_left: poll::parse_keybinding(keyconfig.wheel_left),
		wheel_right: poll::parse_keybinding(keyconfig.wheel_right),
	};
}
