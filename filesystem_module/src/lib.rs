#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(static_mut_refs)]

use std::{collections::HashMap, ffi::c_void, iter, ptr, slice, sync::{OnceLock, RwLock}, u32};
use windows_sys::{
    Wdk::{Foundation::{NtClose, OBJECT_ATTRIBUTES}, Storage::FileSystem::{FILE_CREATE, FILE_DIRECTORY_INFORMATION, FILE_DISPOSITION_DELETE, FILE_DISPOSITION_INFORMATION_EX, FILE_DISPOSITION_POSIX_SEMANTICS, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_IF, FILE_SYNCHRONOUS_IO_NONALERT, FileDirectoryInformation, NtCreateFile, NtOpenFile, NtQueryDirectoryFile, NtReadFile, NtSetInformationFile, NtWriteFile, RtlCreateUnicodeString}, System::SystemServices::{MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE}}, Win32::{Foundation::*, Storage::FileSystem::{CreateFileW, DELETE, DISK_SPACE_INFORMATION, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_DATA, FILE_SHARE_NONE, FILE_SHARE_READ, FILE_WRITE_DATA, GetDiskSpaceInformationW, GetDriveTypeW, GetLogicalDriveStringsW, OPEN_ALWAYS, OPEN_EXISTING, SYNCHRONIZE}, System::{Diagnostics::Debug::{FORMAT_MESSAGE_ALLOCATE_BUFFER, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW}, IO::IO_STATUS_BLOCK, Memory::VirtualAlloc}}
};

/*
references:
https://learn.microsoft.com/en-us/windows/win32/secauthz/access-mask#object-specific-access-rights
https://learn.microsoft.com/en-us/windows/win32/FileIO/file-access-rights-constants
https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntopenfile
*/


#[repr(u16)]
pub enum MESSAGE_CODE {
    FILE_COPY     = 3,
	FILE_MOVE     = 3<<8 | 1,
	FILE_DELETE   = 3<<8 | 2,
	FILE_UPLOAD   = 3<<8 | 3,
	FILE_DOWNLOAD = 3<<8 | 4,
	
    DIRECTORY_PRINT  = 4,
	DIRECTORY_CHANGE = 4<<8 | 1,
	DIRECTORY_CREATE = 4<<8 | 2,
	DIRECTORY_COPY   = 4<<8 | 3,
	DIRECTORY_MOVE   = 4<<8 | 4,
	DIRECTORY_LIST   = 4<<8 | 5,
	DIRECTORY_DELETE = 4<<8 | 6,

    DRIVE_LIST = 5<<8|5,
}

#[repr(u16)] 
pub enum MESSAGE_FLAGS {
	MESSAGE_FLAG_NONE               =  0,
	MESSAGE_FLAG_ERROR                  = 1,
	MESSAGE_FLAG_JOB                    = 2,
	MESSAGE_FLAG_MESSAGE_FRAGMENT       = 4,
	MESSAGE_FLAG_MESSAGE_FRAGMENT_FINAL = 8,
	MESSAGE_FLAG_BODY_FORMAT_JSON       = 16,
}

const FileDispositionInformation:i32 = 13;
const FileDispositionInformationEx:i32 = 64;
const FileRenameInformationEx:i32 = 65;

struct proc {
    flags: u8,
    message_code:u16, //message code if the flags indicate its a builtin.
    address: usize,
    name_len:u32,
    name:[u16;16],
}

//type SendData = extern "system" fn (header:*const u8,body:*const u8,body_len:usize) -> usize;
//static SendDataProc: OnceLock<SendData> = OnceLock::new();

//open_file_handle needs the message label because if a user downloads again while one download request is being processesd it could corrupt state.
#[derive(Clone)]
struct open_file_handle {
    handle:usize,
    offset:i64,
}

//input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize

static OPEN_FILE_HANDLES: OnceLock<RwLock<HashMap<u32, open_file_handle>>> = OnceLock::new();

#[unsafe(no_mangle)]
pub unsafe fn register(internal_proc_list_ptr:*const usize, internal_proc_list_len:*const usize,module_proc_list_ptr:*mut usize,module_proc_list_len:*mut usize) -> i32 {
    /*
    let mut offset:usize = 0;
    loop {
        let ptr = internal_proc_list_ptr as *const c_void;
        let current_internal_proc = ptr.add(offset).cast::<proc>();

        if (*current_internal_proc).address != 0 {
            let name = String::from_utf16(slice::from_raw_parts(&(*current_internal_proc).name as *const _,(*current_internal_proc).name_len as usize)).unwrap();
            if name == "SendData" {
                SendDataProc.set(std::mem::transmute((*current_internal_proc).address)).expect("already initialized");
            }
        } else {
            break
        }
        offset = offset + size_of::<proc>();
    } 
    */
    //return this modules available functions + meta data.
    let mut proc_list:Vec<proc> = Vec::new();

    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::FILE_UPLOAD as u16, address: file_upload as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::FILE_DOWNLOAD as u16, address: file_download as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::DIRECTORY_LIST as u16, address: directory_list as usize, name_len: 0, name: [0;16] });
    //proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::FILE_COPY as u16, address: file_copy as usize, name_len: 0, name: [0;16] });
    //proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::FILE_MOVE as u16, address: file_rename as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::DRIVE_LIST as u16, address: drives as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: MESSAGE_CODE::FILE_DELETE as u16, address: file_remove as usize, name_len: 0, name: [0;16] });
    proc_list.push(proc { flags: 0, message_code: 0, address: 0, name_len: 0, name: [0;16] });

    let out_proc_list_len = size_of::<proc>()*proc_list.len();
    *module_proc_list_len = out_proc_list_len;

    let out_proc_list = VirtualAlloc(std::ptr::null(), out_proc_list_len, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
    ptr::copy_nonoverlapping(proc_list.as_ptr() as *const _, out_proc_list, out_proc_list_len);
    ptr::write(module_proc_list_ptr, out_proc_list as usize);
    0
}

fn global() -> &'static RwLock<HashMap<u32, open_file_handle>> {
    OPEN_FILE_HANDLES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn insert_kv(k: impl Into<u32>, v: impl Into<open_file_handle>) {
    let lock = global();
    let mut map = lock.write().unwrap();
    map.insert(k.into(), v.into());
}

fn get_k(k: u32) -> Option<open_file_handle> {
    let lock = global();
    let map = lock.read().unwrap();
    map.get(&k).cloned()
}

fn delete_k(k: u32) {
    let lock = global();
    let mut map = lock.write().unwrap();
    map.remove(&k);
}

pub unsafe fn NtCreateFile_wrapper(filehandle:*mut HANDLE, path:*const u16,path_len:usize,desired_access:u32,share_access:u32,CreateDisposition:u32,create_options:u32) -> i32 {
    let path = String::from_utf16(slice::from_raw_parts(path,path_len/2)).unwrap();
    let path:Vec<u16> = path.encode_utf16().chain(iter::once(0)).collect();

    let mut path_unicode_string:UNICODE_STRING = UNICODE_STRING::default();
    RtlCreateUnicodeString(&mut path_unicode_string, path.as_ptr());
    
    let mut obj_atr = OBJECT_ATTRIBUTES{
        Length:size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: HANDLE::default(),
        ObjectName: &path_unicode_string,
        Attributes:OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),

    };

    let mut iostatus:IO_STATUS_BLOCK = Default::default();
    let status: NTSTATUS = NtCreateFile(&mut *filehandle, desired_access, &mut obj_atr, &mut iostatus, std::ptr::null_mut(), FILE_ATTRIBUTE_NORMAL, share_access, CreateDisposition, create_options, std::ptr::null_mut(), 0);
    status
}

pub unsafe fn NtOpenFile_wrapper(filehandle:*mut HANDLE, path:*const u16,path_len:usize,desired_access:u32,share_access:u32,open_options:u32) -> i32 {
    let path = String::from_utf16(slice::from_raw_parts(path,path_len/2)).unwrap();
    let path:Vec<u16> = path.encode_utf16().chain(iter::once(0)).collect();

    let mut path_unicode_string:UNICODE_STRING = UNICODE_STRING::default();
    RtlCreateUnicodeString(&mut path_unicode_string, path.as_ptr());
    
    let mut obj_atr = OBJECT_ATTRIBUTES{
        Length:size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: HANDLE::default(),
        ObjectName: &path_unicode_string,
        Attributes:OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),

    };

    let mut iostatus:IO_STATUS_BLOCK = Default::default();
    let status: NTSTATUS = NtOpenFile(&mut *filehandle, desired_access, &mut obj_atr, &mut iostatus, share_access, open_options);
    status
}
/*
    pub SectorsPerAllocationUnit: u32,
    pub BytesPerSector: u32,
*/
fn display_bytes(measurement:u64,SectorsPerAllocationUnit: u32,BytesPerSector: u32) -> String {
    let bytes = measurement * SectorsPerAllocationUnit as u64 * BytesPerSector as u64;
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

// External function declaration for RtlGetCurrentDirectory_U
unsafe extern "system" {
    fn RtlGetCurrentDirectory_U(buffer_length: u32, buffer: *mut u16) -> u32;
}

// Get current directory using native API
pub unsafe fn get_current_directory() -> Result<String, i32> {
    // First call to get the required buffer size
    let required_length = RtlGetCurrentDirectory_U(0, ptr::null_mut());
    
    if required_length == 0 {
        return Err(-1);
    }
    
    // Allocate buffer (length includes null terminator)
    let buffer_size = (required_length + 2) as usize; // +2 for safety
    let mut buffer: Vec<u16> = vec![0; buffer_size / 2];
    
    // Second call to get the actual directory
    let actual_length = RtlGetCurrentDirectory_U(buffer_size as u32, buffer.as_mut_ptr());
    
    if actual_length == 0 || actual_length > required_length {
        return Err(-2);
    }
    
    // Convert to String (actual_length is in bytes, so divide by 2 for UTF-16 chars)
    let char_count = (actual_length / 2) as usize;
    buffer.truncate(char_count);
    
    String::from_utf16(&buffer).map_err(|_| -3)
}

//https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskspaceinformationw
//https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew
//https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getlogicaldrivestringsw
#[unsafe(no_mangle)]
pub unsafe fn drives(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let mut drives:Vec<u8> = Vec::with_capacity(512);
    let len = GetLogicalDriveStringsW(512, drives.as_mut_ptr() as *mut u16);
    drives.set_len(len as usize * 2);
    let mut json_output = String::from("{\"drives\":[");
    let mut offset :usize = 0;
    for i in 0..drives.len() {
        if drives[i] == 0 && drives[i+1] == 0 {
            let disk_space_info:Vec<u8> = Vec::with_capacity(size_of::<DISK_SPACE_INFORMATION>());
            GetDiskSpaceInformationW(drives.as_ptr().add(offset) as *const u16, disk_space_info.as_ptr() as *mut _);
            let disk_space_info = disk_space_info.as_ptr().cast::<DISK_SPACE_INFORMATION>();
            let space_used = display_bytes((*disk_space_info).UsedAllocationUnits, (*disk_space_info).SectorsPerAllocationUnit, (*disk_space_info).BytesPerSector);
            let space_total = display_bytes((*disk_space_info).ActualTotalAllocationUnits, (*disk_space_info).SectorsPerAllocationUnit, (*disk_space_info).BytesPerSector);
            let drive_name = String::from_utf16(slice::from_raw_parts(drives.as_ptr().add(offset) as *const u16, 3)).unwrap();
            let drive_name = drive_name.replace("\\","\\\\").replace("\"","\\\"");
            let drive_type = GetDriveTypeW(drives.as_ptr().add(offset) as *const u16);
            let drive_type = match drive_type {
                0 => "UNKNOWN",
                1 => "No Root Directory",
                2 => "Removable Disk",
                3 => "Fixed Disk",
                4 => "Remote Disk",
                5 => "CD-ROM",
                6 => "RAM Disk",
                _ => "Invalid",
            };
            json_output = json_output + &format!("{{\"name\":\"{drive_name}\",\"type\":\"{drive_type}\",\"used\":\"{space_used}\",\"total\":\"{space_total}\"}}");
            
            if drives[i+2] == 0 {
                break;
            }
            json_output = json_output + ",";
            offset = i + 2;
        }
    }
    let json_output = json_output + "]}";
    let json_output_utf16:Vec<u16> = json_output.encode_utf16().collect();

    *out_size = json_output_utf16.len() * 2;
    let tmp = VirtualAlloc(std::ptr::null(), *out_size, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE) as *mut u8;
    ptr::copy_nonoverlapping(json_output_utf16.as_ptr() as *const u8, tmp, *out_size);
    ptr::write(out, tmp as usize);
    0
}


#[unsafe(no_mangle)]
pub unsafe fn file_upload(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let status:i32;
    let offset:i64;
    let mut src_handle:HANDLE = HANDLE::default();

    match get_k(label) {
        Some(v) => {
            offset = v.offset;
            src_handle = v.handle as *mut _;
        },
        None => {
            let target_file_string = String::from_utf16(slice::from_raw_parts(input as *const u16,input_size/2)).unwrap();
            status = NtCreateFile_wrapper(&mut src_handle, input as *const u16, input_size, SYNCHRONIZE|FILE_READ_DATA|FILE_WRITE_DATA, 0,FILE_OPEN_IF,FILE_SYNCHRONOUS_IO_NONALERT|FILE_NON_DIRECTORY_FILE);
            if status != ERROR_ALREADY_EXISTS as i32 && status != 0 {
                return status;
            }
            insert_kv(label, open_file_handle{handle:src_handle as usize,offset:0});
            return 0;
        }
    };

    let mut iostatus:IO_STATUS_BLOCK = Default::default();
    status = NtWriteFile(src_handle,
        std::ptr::null_mut(),
        None,
        std::ptr::null_mut(),
        &mut iostatus,
        input as *const _,
        input_size as u32,
        &offset,
        std::ptr::null_mut());
    
    insert_kv(label, open_file_handle{handle:src_handle as usize,offset:offset+input_size as i64});
    if (*flags & MESSAGE_FLAGS::MESSAGE_FLAG_MESSAGE_FRAGMENT_FINAL  as u16)!= 0 {
        NtClose(src_handle);
        delete_k(label);
    } 

    status

}

#[unsafe(no_mangle)]
pub unsafe fn file_download(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    
    let status:i32;
    let offset:i64;
    let mut src_handle:HANDLE = HANDLE::default();

    match get_k(label) {
        Some(v) => {
            offset = v.offset;
            src_handle = v.handle as *mut _;
        },
        None => {
            status = NtOpenFile_wrapper(&mut src_handle, input as *const u16, input_size, SYNCHRONIZE|FILE_READ_DATA, FILE_OPEN, FILE_SYNCHRONOUS_IO_NONALERT|FILE_NON_DIRECTORY_FILE);
            if status != 0 {
                return status;
            }
            insert_kv(label, open_file_handle{handle:src_handle as usize,offset:0});
            return 0;
        }
    };

    let mut iostatus:IO_STATUS_BLOCK = Default::default();


    *out_size = 4096;
    let tmp = VirtualAlloc(std::ptr::null(), *out_size, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
    ptr::write(out, tmp as usize);

    status = NtReadFile(src_handle,
        std::ptr::null_mut(),
        None,
        std::ptr::null_mut(),
        &mut iostatus,
         tmp,
         4096,
         &offset,
         std::ptr::null_mut());

    *out_size = iostatus.Information as usize;
    insert_kv(label, open_file_handle{handle:src_handle as usize,offset:offset+*out_size as i64});
    if *out_size < 4096 {
        *flags = MESSAGE_FLAGS::MESSAGE_FLAG_MESSAGE_FRAGMENT_FINAL as u16;
        NtClose(src_handle);
        delete_k(label);
    }

    status
}

fn vec_u16_to_vec_u8_be(v: Vec<u16>) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 2);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

#[unsafe(no_mangle)]
pub unsafe fn directory_list(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let mut iostatus:IO_STATUS_BLOCK = Default::default();

    let input_directory = String::from_utf16(slice::from_raw_parts(input as *const u16,input_size/2)).unwrap();

    let search_directory_utf16:Vec<u16> = input_directory.encode_utf16().chain(iter::once(0)).collect(); 
    let directory_handle =  CreateFileW(search_directory_utf16.as_ptr() as *const u16, GENERIC_READ, FILE_SHARE_READ, std::ptr::null_mut(),OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS, std::ptr::null_mut());
    if directory_handle as usize == 0xffffffffffffffff  {
        let error_out = output_last_error(None);
        let error_out:Vec<u16> = error_out.encode_utf16().collect();

        *out_size = error_out.len() * 2;
        let tmp = VirtualAlloc(std::ptr::null(), *out_size, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE) as *mut u8;
        ptr::copy_nonoverlapping(error_out.as_ptr() as *const u8, tmp, *out_size);
        ptr::write(out, tmp as usize);

        *flags = 1|16;
        return 0
    }
    let mut size:u32 = 512;
    let mut repeat:bool = false;
    let mut buffer: Vec<u8>;
    loop {
        buffer = vec![0; size as usize];
        let status = NtQueryDirectoryFile(directory_handle,std::ptr::null_mut(),None,std::ptr::null(),&mut iostatus,buffer.as_mut_ptr() as *mut std::ffi::c_void,size,FileDirectoryInformation,false,std::ptr::null(),repeat);
        if repeat {
            break
        }
        if status == STATUS_NO_MORE_FILES {
            repeat = true;
            continue;
        }
        size = size * 2;
    }

    let mut json_output = format!("{{\"path\":\"{}\",\"files\":[", hex_encode(&input_directory));
    let mut offset:usize = 0;

    loop {
        let file_directory_info_entry = buffer.as_ptr().add(offset).cast::<FILE_DIRECTORY_INFORMATION>();
        
        let filename= String::from_utf16(slice::from_raw_parts((*file_directory_info_entry).FileName.as_ptr(), (*file_directory_info_entry).FileNameLength as usize /2)).unwrap();
        json_output = json_output + &format!("{{\"name\":\"{}\",\"size\":\"{}\",\"attributes\":{},\"last_written\":{}}}", filename, (*file_directory_info_entry).EndOfFile, (*file_directory_info_entry).FileAttributes,(*file_directory_info_entry).LastWriteTime);

        if (*file_directory_info_entry).NextEntryOffset == 0 {
            break
        }

        json_output = json_output + ",";
        offset += (*file_directory_info_entry).NextEntryOffset as usize;
    }

    let json_output = json_output + "]}";
    let json_output_utf16:Vec<u16> = json_output.encode_utf16().collect();
    
    *out_size = json_output_utf16.len() * 2;
    let tmp = VirtualAlloc(std::ptr::null(), *out_size, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE) as *mut u8;
    ptr::copy_nonoverlapping(json_output_utf16.as_ptr() as *const u8, tmp, *out_size);
    ptr::write(out, tmp as usize);
    0
}

//how to handle mutiple parameters within the single argument is tbd.
/*
//copy file using hardlink technique
//https://ntdoc.m417z.com/ntsetinformationfile + FileLinkInformation
#[unsafe(no_mangle)]
pub unsafe fn file_copy(input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let mut src_handle:HANDLE = HANDLE::default();
    let mut status = NtOpenFile_wrapper(&mut src_handle, src_path, src_path_len, SYNCHRONIZE|FILE_READ_DATA, FILE_OPEN, FILE_SYNCHRONOUS_IO_NONALERT|FILE_NON_DIRECTORY_FILE);
    if status != 0 {
        return status;
    }

    let info = FILE_LINK_INFORMATION{
        Anonymous:FILE_LINK_INFORMATION_0 {ReplaceIfExists:true},
        RootDirectory:HANDLE::default(),
        FileNameLength:dst_path_len as u32,
        FileName:[0;1],
    };

    let mut buf = vec![0u8; size_of::<FILE_LINK_INFORMATION>()+dst_path_len];
    unsafe {
        let src = &info as *const FILE_LINK_INFORMATION as *const u8;
        ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), size_of::<FILE_LINK_INFORMATION>());
        ptr::copy_nonoverlapping(dst_path as *mut u8, buf.as_mut_ptr().add(0x14), dst_path_len);
    }

    let mut iostatus:IO_STATUS_BLOCK = Default::default();
    status = NtSetInformationFile(src_handle,
        &mut iostatus,
        buf.as_ptr() as *const _,
        (size_of::<FILE_LINK_INFORMATION>() + dst_path_len) as u32,
        FileLinkInformation);
    NtClose(src_handle);
    status
}
*/
/*
//https://ntdoc.m417z.com/ntsetinformationfile + FileRenameInformationEx
#[unsafe(no_mangle)]
pub unsafe fn file_rename(input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let mut src_handle:HANDLE = HANDLE::default();
    let mut status = NtOpenFile_wrapper(&mut src_handle, src_path, src_path_len, DELETE, FILE_SHARE_NONE, FILE_ATTRIBUTE_NORMAL);
    if status != 0 {
        return status;
    }

    let info = FILE_RENAME_INFORMATION{
        Anonymous: FILE_RENAME_INFORMATION_0 { ReplaceIfExists: true },
        RootDirectory:HANDLE::default(),
        FileNameLength:dst_path_len as u32,
        FileName:[0;1],
    };

    let mut buf = vec![0u8;size_of::<FILE_RENAME_INFORMATION>()+dst_path_len];
    unsafe {
        let src = &info as *const FILE_RENAME_INFORMATION as *const u8;
        ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), size_of::<FILE_RENAME_INFORMATION>());
        ptr::copy_nonoverlapping(dst_path as *mut u8, buf.as_mut_ptr().add(0x14), dst_path_len);
    }

    let mut iostatus:IO_STATUS_BLOCK = Default::default();
    status = NtSetInformationFile(src_handle,
        &mut iostatus,
        buf.as_ptr() as *const _,
        (size_of::<FILE_RENAME_INFORMATION>() + dst_path_len) as u32,
        FileRenameInformationEx);
    NtClose(src_handle);
    status
}
*/
//https://ntdoc.m417z.com/ntsetinformationfile + FileDispositionInfoEx
#[unsafe(no_mangle)]
pub unsafe fn file_remove(flags: *mut u16,label:u32,input: *const usize,input_size:usize,out: *mut usize,out_size: *mut usize) -> i32 {
    let path = String::from_utf16(slice::from_raw_parts(input as *const u16, input_size/2)).unwrap();
    let status = remove_path_recursive(&path);
    
    if status != 0 {
        let error_out = output_last_error(Some(format!("Failed to remove: {}", path)));
        let error_out:Vec<u16> = error_out.encode_utf16().collect();

        *out_size = error_out.len() * 2;
        let tmp = VirtualAlloc(std::ptr::null(), *out_size, MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE) as *mut u8;
        ptr::copy_nonoverlapping(error_out.as_ptr() as *const u8, tmp, *out_size);
        ptr::write(out, tmp as usize);

        *flags = 1|16;
    }
    status
}

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_DIRECTORY_FILE: u32 = 0x00000001;

unsafe fn remove_path_recursive(path: &str) -> i32 {
    // First, try to open as a directory to check if it is one
    let path_utf16: Vec<u16> = path.encode_utf16().chain(iter::once(0)).collect();
    let directory_handle = CreateFileW(
        path_utf16.as_ptr(),
        GENERIC_READ | DELETE,
        FILE_SHARE_READ,
        std::ptr::null_mut(),
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS,
        std::ptr::null_mut()
    );
    
    if directory_handle as usize == 0xffffffffffffffff {
        // Could not open - return error
        return -1;
    }

    // Check if this is a directory by querying its contents
    let mut iostatus: IO_STATUS_BLOCK = Default::default();
    let mut buffer: Vec<u8> = vec![0; 8192];
    let status = NtQueryDirectoryFile(
        directory_handle,
        std::ptr::null_mut(),
        None,
        std::ptr::null(),
        &mut iostatus,
        buffer.as_mut_ptr() as *mut std::ffi::c_void,
        buffer.len() as u32,
        FileDirectoryInformation,
        false,
        std::ptr::null(),
        false
    );
    
    NtClose(directory_handle);

    if status == STATUS_NOT_A_DIRECTORY {
        // It's a file, delete it directly
        return delete_single_file(path);
    }
    
    if status != 0 && status != STATUS_NO_MORE_FILES {
        // Some other error, try deleting as a file anyway
        return delete_single_file(path);
    }

    // It's a directory - enumerate and delete contents first
    let directory_handle = CreateFileW(
        path_utf16.as_ptr(),
        GENERIC_READ,
        FILE_SHARE_READ,
        std::ptr::null_mut(),
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS,
        std::ptr::null_mut()
    );
    
    if directory_handle as usize == 0xffffffffffffffff {
        return -1;
    }

    // Collect all entries first
    let mut entries: Vec<(String, bool)> = Vec::new();
    let mut size: u32 = 512;
    let mut restart = true;
    
    loop {
        let mut buffer: Vec<u8> = vec![0; size as usize];
        let mut iostatus: IO_STATUS_BLOCK = Default::default();
        let status = NtQueryDirectoryFile(
            directory_handle,
            std::ptr::null_mut(),
            None,
            std::ptr::null(),
            &mut iostatus,
            buffer.as_mut_ptr() as *mut std::ffi::c_void,
            size,
            FileDirectoryInformation,
            false,
            std::ptr::null(),
            restart
        );
        restart = false;
        
        if status == STATUS_NO_MORE_FILES {
            break;
        }
        
        if status == STATUS_BUFFER_OVERFLOW || status == STATUS_INFO_LENGTH_MISMATCH {
            size = size * 2;
            restart = true;
            continue;
        }
        
        if status != 0 {
            break;
        }

        // Parse directory entries
        let mut offset: usize = 0;
        loop {
            let entry = buffer.as_ptr().add(offset).cast::<FILE_DIRECTORY_INFORMATION>();
            let filename = String::from_utf16(slice::from_raw_parts(
                (*entry).FileName.as_ptr(),
                (*entry).FileNameLength as usize / 2
            )).unwrap();
            
            // Skip . and ..
            if filename != "." && filename != ".." {
                let is_dir = ((*entry).FileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
                entries.push((filename, is_dir));
            }
            
            if (*entry).NextEntryOffset == 0 {
                break;
            }
            offset += (*entry).NextEntryOffset as usize;
        }
    }
    
    NtClose(directory_handle);

    // Recursively delete all entries
    for (name, is_dir) in entries {
        let child_path = if path.ends_with('\\') {
            format!("{}{}", path, name)
        } else {
            format!("{}\\{}", path, name)
        };
        
        let status = if is_dir {
            remove_path_recursive(&child_path)
        } else {
            delete_single_file(&child_path)
        };
        
        if status != 0 {
            return status;
        }
    }

    // Now delete the empty directory
    delete_single_file(path)
}

unsafe fn delete_single_file(path: &str) -> i32 {
    // Convert to NT path format for NtOpenFile
    let nt_path = if path.starts_with("\\??\\") || path.starts_with("\\\\?\\") {
        path.to_string()
    } else {
        format!("\\??\\{}", path)
    };
    
    let path_utf16: Vec<u16> = nt_path.encode_utf16().chain(iter::once(0)).collect();
    
    let mut handle: HANDLE = HANDLE::default();
    let mut status = NtOpenFile_wrapper(
        &mut handle,
        path_utf16.as_ptr(),
        path_utf16.len() * 2 - 2, // exclude null terminator
        DELETE | SYNCHRONIZE,
        FILE_SHARE_READ,
        FILE_SYNCHRONOUS_IO_NONALERT
    );
    
    if status != 0 {
        return status;
    }

    let info = FILE_DISPOSITION_INFORMATION_EX {
        Flags: FILE_DISPOSITION_DELETE | FILE_DISPOSITION_POSIX_SEMANTICS,
    };

    let mut iostatus: IO_STATUS_BLOCK = Default::default();
    status = NtSetInformationFile(
        handle,
        &mut iostatus,
        &info as *const _ as *const _,
        size_of::<FILE_DISPOSITION_INFORMATION_EX>() as u32,
        FileDispositionInformationEx
    );

    NtClose(handle);
    status
}

//{"message":"","code":0,"context":"","hex_encoded":bool}
pub fn output_last_error(error_context:Option<String>) -> String {
    unsafe {
    let dw = GetLastError();

    let mut output_addr:usize = 0;
    let lpmsg:*mut usize = &mut output_addr;

    let s = FormatMessageW(FORMAT_MESSAGE_ALLOCATE_BUFFER|FORMAT_MESSAGE_FROM_SYSTEM|FORMAT_MESSAGE_IGNORE_INSERTS,
        std::ptr::null(),
        dw,
        0x400,//MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT)
        lpmsg as _,
        0,
        std::ptr::null(),
    );

    let out = String::from_utf16(slice::from_raw_parts(output_addr as _, s as usize)).unwrap();
    LocalFree(output_addr as _);
    match error_context {
        Some(error_context) => {format!(r#"{{"message":"{}","code":{},"context":"{}","hex_encoded":true}}"#,hex_encode(&out),dw,hex_encode(&error_context))},
        None => {format!(r#"{{"message":"{}","code":{},"context":"","hex_encoded":true}}"#,hex_encode(&out),dw)},
    }
    }
}

fn hex_encode(s: &str) -> String {
    s.encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .map(|b| format!("{:02x}", b))
        .collect()
}