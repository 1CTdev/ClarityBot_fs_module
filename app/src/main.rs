use core::slice;
use std::{ffi::c_void, ptr};

use windows_sys::{ Win32::{Storage::FileSystem::FileDispositionInfo, System::{LibraryLoader::{GetProcAddress, LoadLibraryW}, Memory::{MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAlloc, VirtualFree}}}, s, w};

#[unsafe(no_mangle)]
pub unsafe fn SendData(_header:*const u8,body:*const u8,body_len:usize) -> usize {
    let raw_slice: *const [u8] = ptr::slice_from_raw_parts(body, body_len);
    let slice: &[u8] = &*raw_slice;

    println!("sent data:\n{:?}",slice.to_vec());
    0
}

struct proc {
    flags: u8,
    message_code:u16, //message code if the flags indicate its a builtin.
    address: usize,
    name_len:u32,
    name:[u16;16],
}

fn slice_to_array(name:&str) -> ([u16;16],u32) {
    let mut v:Vec<u16> = Vec::with_capacity(16);
    v = name.encode_utf16().collect();
    let len = v.len(); 
    unsafe {v.set_len(16); }

    let mut arr:[u16;16] = [0;16];
    arr.copy_from_slice(&v);
    (arr,len as u32)
}


fn main() {
    unsafe {
    let m = LoadLibraryW(w!("C:\\Users\\user\\source\\repos\\dream_loader\\dream_loader_filesystem_module\\target\\debug\\filesystem_module.dll"));
    
    let mut drives:extern "system" fn (input_directory:*const u16, input_directory_len:usize, out_ptr: *mut usize, out_len:*mut usize) -> usize = std::mem::transmute(GetProcAddress(m, s!("drives")).unwrap() as *const ());
    
    let mut out_len:usize = 0;  
    let mut out: usize = 0;// *mut u8 = std::ptr::
    drives(w!("."), 0, &mut out, &mut out_len);
    let s: &[u8] = slice::from_raw_parts(out as *const _, out_len);
    println!("{:?}",s);
    
    let mut directory_list:extern "system" fn (input_directory:*const u16, input_directory_len:usize, out_ptr: *mut usize, out_len:*mut usize) -> usize = std::mem::transmute(GetProcAddress(m, s!("directory_list")).unwrap() as *const ());
    let mut out_len:usize = 0;  
    let mut out: usize = 0;// *mut u8 = std::ptr::
    directory_list(w!("C:"), 4, &mut out, &mut out_len);
    let s: &[u8] = slice::from_raw_parts(out as *const _, out_len);
    /*
    let directory_list:extern "system" fn (input_directory:*const u16, input_directory_len:usize, out_ptr: *mut usize, out_len:*mut usize) -> usize = std::mem::transmute(GetProcAddress(m, s!("directory_list")).unwrap() as *const ());
    let file_upload:extern "system" fn (target_file_path:*const u16,target_file_path_len:usize,input_content_ptr: *mut usize,input_content_len:*mut usize) -> usize = std::mem::transmute(GetProcAddress(m,s!("file_upload")).unwrap() as *const ());
    let file_download:extern "system" fn (target_file_path:*const u16,target_file_path_len:usize,out_ptr: *mut usize,out_len:*mut usize) -> usize = std::mem::transmute(GetProcAddress(m,s!("file_download")).unwrap() as *const ());
    let file_copy:extern "system" fn (src_path:*const u16,src_path_len:usize,dst_path:*const u16,dst_path_len:usize,out_ptr: *mut usize,out_len:*mut usize) -> usize = std::mem::transmute(GetProcAddress(m, s!("file_copy")).unwrap() as *const ());
    let file_remove:extern "system" fn (src_path:*const u16,src_path_len:usize,fileinformationclass:i32) -> usize = std::mem::transmute(GetProcAddress(m, s!("file_remove")).unwrap() as *const ());
    let file_rename:extern "system" fn (src_path:*const u16,src_path_len:usize,dst_path:*const u16,dst_path_len:usize,out_ptr: *mut usize,out_len:*mut usize) -> usize = std::mem::transmute(GetProcAddress(m, s!("file_rename")).unwrap() as *const ());
    */
    let register:extern "system" fn (internal_proc_list_ptr:*const usize, internal_proc_list_len:*const usize,module_proc_list_ptr:*mut usize,module_proc_list_len:*mut usize) -> usize = std::mem::transmute(GetProcAddress(m,s!("register")).unwrap() as *const ());
    

    let mut file_rename: extern "system" fn (src_path:*const u16,src_path_len:usize,dst_path:*const u16,dst_path_len:usize,out_ptr: *mut usize,out_len:*mut usize) -> usize;
    let mut file_copy:extern "system" fn (src_path:*const u16,src_path_len:usize,dst_path:*const u16,dst_path_len:usize,out_ptr: *mut usize,out_len:*mut usize) -> usize;
    let mut directory_list:extern "system" fn (input_directory:*const u16, input_directory_len:usize, out_ptr: *mut usize, out_len:*mut usize) -> usize;

    /*
        proc_list.push(proc { flags: 0, message_code: 0, address: file_upload as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: 0, address: file_download as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: 0, address: directory_list as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: 8<<3 | 0, address: file_copy as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: 8<<3 | 2, address: file_rename as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: 0, address: file_remove as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: 0, address: 0, name_len: 0, name: [0;16] });

    let out_proc_list_len = size_of::<proc>()*proc_list.len();
    *module_proc_list_len = out_proc_list_len;

    let out_proc_list = VirtualAlloc(std::ptr::null(), out_proc_list_len, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
    ptr::copy_nonoverlapping(proc_list.as_ptr() as *const _, out_proc_list, out_proc_list_len);
    ptr::write(module_proc_list_ptr, out_proc_list as usize);
     */

    //input to register

    let mut proc_list:Vec<proc> = Vec::new();
    let (p_name,p_len) = slice_to_array("SendData");
    proc_list.push(proc { flags: 0, message_code: 0, address: SendData as usize, name_len: p_len, name: p_name });
    proc_list.push(proc { flags: 0, message_code: 0, address: 0, name_len: 0, name: [0;16] });

    let internal_proc_list_len:usize = size_of::<proc>()*proc_list.len();
    let internal_proc_list = VirtualAlloc(std::ptr::null(), internal_proc_list_len, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
    
    ptr::copy_nonoverlapping(proc_list.as_ptr() as *const _, internal_proc_list, internal_proc_list_len);

    //output from register
    let mut module_proc_list:usize = 0;
    let mut module_proc_list_len:usize = 0;

    register(internal_proc_list as *const _,&internal_proc_list_len,&mut module_proc_list,&mut module_proc_list_len);
    VirtualFree(internal_proc_list,0,MEM_RELEASE);
    //println!("{}{}",proc_list,proc_list_len);
    let mut offset:usize = 0;
    loop {
        let ptr = module_proc_list as *const c_void;
        let f = ptr.add(offset).cast::<proc>();

        if (*f).address == 0 || offset > module_proc_list_len {
            break;
        }
        if (*f).message_code == 8<<4 | 5 {
            directory_list = std::mem::transmute((*f).address);
                let mut out_len:usize = 0;  
                let mut out: usize = 0;// *mut u8 = std::ptr::null_mut();
                directory_list(w!("."), 4, &mut out, &mut out_len);
        }
        /*
        if (*f).message_code == 8<<3 | 2 {
            file_rename = std::mem::transmute((*f).address);
            println!("set file_rename");

            let mut out_len:usize = 0;  
            let mut out: usize = 0;
            let status = file_rename(w!("\\??\\\\C:\\temp\\test.txt"),42, w!("\\??\\\\C:\\temp\\testfolder\\test2.txt"),66, &mut out, &mut out_len);
            println!("{:x}",status);
        } else if (*f).message_code == 8<<3 | 0 {
            file_copy = std::mem::transmute((*f).address);
            let mut out_len:usize = 0;  
            let mut out: usize = 0;
            
            let status = file_copy(w!("\\??\\\\C:\\temp\\test.txt"),42, w!("\\??\\\\C:\\temp\\testfolder\\test3.txt"),66, &mut out, &mut out_len);
            println!("{:x}",status);
        }
        */
        println!("flags: {:x} message code: {:x} address: {:x} name_len: {:x}",(*f).flags,(*f).message_code,(*f).address,(*f).name_len);
        offset = offset + size_of::<proc>();

    }

    println!("done");
    /*
    let mut out_len:usize = 0;  
    let mut out: usize = 0;// *mut u8 = std::ptr::null_mut();
    directory_list(w!("."), 4, &mut out, &mut out_len);
    //add additional flag to server to indicate parse structure directly instead of json.
    let s: &[u8] = slice::from_raw_parts(out as *const _, out_len);
    println!("{:?}",s);

    
    let mut out_len:usize = 0;  
    let mut out: usize = 0;// *mut u8 = std::ptr::null_mut();
    file_copy(w!("\\??\\\\C:\\temp\\test.txt"),42, w!("\\??\\\\C:\\temp\\testfolder\\test2.txt"),66, &mut out, &mut out_len);//can't have extract null terminators or function will fail.
    //let s: &[u8] = slice::from_raw_parts(out as *const _, out_len/2);
    //println!("{:?}",s);
    

    
    
    //let s = file_remove(w!("\\??\\\\C:\\temp\\test.txt"),42,13);
    //println!("{:x}",s);
    */
    }
    
}
