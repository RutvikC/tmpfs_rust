extern crate fuse;
extern crate libc;
extern crate time;
#[macro_use]
extern crate log;
extern crate env_logger;

use std::iter;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use libc::{ENOENT, EINVAL, EEXIST, ENOTEMPTY, EFBIG};
use fuse::{FileAttr, FileType, Filesystem, Request,
    ReplyAttr, ReplyData, ReplyEntry, ReplyDirectory,
    ReplyEmpty, ReplyWrite, ReplyOpen, ReplyCreate};
use time::Timespec; // This library is used to get system-time

#[derive(Debug, Clone, Default)]
pub struct File {
    data: Vec<u8>, // had to change this because append_data is a vector of 8-bit unsigned int
    is_updated: bool,
    old_size: i64,
}

impl File {
    /* Creates a new attribute for File*/
    fn new_file() -> File {
        File{data: Vec::new(), is_updated: false, old_size: 0,}
    }

    /* Returns the number of bytes of data in the file*/
    fn get_file_size(&self) -> u64 {
        self.data.len() as u64 // Returning a unsigned 64 integer because that's what the FileAttr needs for size
    }

    /* Appends the file with new data at a specific offset*/
    fn update_file(&mut self, offset: i64, append_data: &[u8]) -> u64 {
        let offset: usize = offset as usize;

        if offset >= self.data.len() {
            self.data.extend(iter::repeat(0).take(offset - self.data.len())); // Extending with 0s until we reach the offset bit
            self.data.splice(offset.., append_data.iter().cloned()); // Basically we are appending the new data after the offset as there is no data after that
        }
        else if offset + append_data.len() > self.data.len() {
            self.data.splice(offset.., append_data.iter().cloned()); // Same as before
        }
        else {
            self.data.splice(offset..offset, append_data.iter().cloned()); // Here we want to append data right after the offset
        }
        append_data.len() as u64
    }

    /* Shortens the vector, keeping the first 'size' elements and dropping the rest. */
    fn truncate_bytes(&mut self, size: u64) {
        self.data.truncate(size as usize);
    }
}

#[derive(Debug, Clone)]
pub struct Inode {
    name: String,
    nodes: BTreeMap<String, u64>, // Binary-tree for all the successive directories/files
    root: u64, // Represents the Inode number for parent directory

    // Most of the Inode attributes mentioned in 'fs.h' would be handled by fuse::FileAttr
}

impl Inode {
    fn new_inode(label: String, parent: u64) -> Inode {
        Inode{name: label, nodes: BTreeMap::new(), root: parent,}
    }
}

pub struct TmpFS {
    files: BTreeMap<u64, File>,
    attrs: BTreeMap<u64, FileAttr>,
    inodes: BTreeMap<u64, Inode>,
    next_inode: u64,
    fs_size: i64,
    fs_max_size: u64,
}

impl TmpFS {
    pub fn new(size: u64) -> TmpFS {
        let files_stub = BTreeMap::new(); 
        let root = Inode::new_inode("/".to_string(), 1 as u64); // Setting '/' as root and assigning Inode value for it to 1
        
        let ts = time::now().to_timespec();
        let mut root_dir_attrs = BTreeMap::new();
        let attr = FileAttr { // Defining attributes for root directory
            ino: 1, //u64 Since the inode-value of our root directory is 1
            size: 0, //u64,
            blocks: 1, //u64,
            atime: ts, //Timespec,
            mtime: ts, //Timespec,
            ctime: ts, //Timespec,
            crtime: ts, //Timespec,
            kind: FileType::Directory, //FileType,
            perm: 0o755, //u16, The initial permission for a directory according to tmpFS in Linux
            nlink: 0, //u32,
            uid: 0, //u32,
            gid: 0, //u32,
            rdev: 0, //u32,
            flags: 0, //u32,
        };
        root_dir_attrs.insert(1, attr); // Updating root-directory attributes

        let mut root_dir_inode = BTreeMap::new();
        root_dir_inode.insert(1, root); // Adding the root directory inode in the FS
        println!("fs_max_size: {}", size);
        TmpFS {
            files: files_stub,
            attrs: root_dir_attrs,
            inodes: root_dir_inode,
            next_inode: 2, // Moving in order after creating the initial root directory
            fs_size: 0,
            fs_max_size: size,
        }
    }

    /* Returns the next inode value in filesystem tree*/
    fn get_next_inode(&mut self) -> u64 { // This is function is straight-up from ramFS in linux
        self.next_inode += 1;
        self.next_inode
    }
}

// Out of all the function that the fuse::FileSystem implements there are handful of them which need tweaking
impl Filesystem for TmpFS {

    /* This function gets the file's attributes for specified 'ino' value */
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match self.attrs.get_mut(&ino) {
            Some(attr) => {
                reply.attr(&Timespec::new(1,0), attr);
            }
            // If no matching inode value found, then throw error
            None => {
                error!("getattr: Cannot find inode: {}", ino);
                reply.error(ENOENT); // File not found error
            },
        };
    }

    /* This function updates the FileType at 'ino' attributes */
    // There are only a handful of attributes that can actually be changed once a FileType is instantiated
    fn setattr(&mut self, _req: &Request, ino: u64, _mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, atime: Option<Timespec>, mtime: Option<Timespec>, _fh: Option<u64>, crtime: Option<Timespec>, _chgtime: Option<Timespec>, _bkuptime: Option<Timespec>, _flags: Option<u32>, reply: ReplyAttr) {
        match self.attrs.get_mut(&ino) {
            // After getting the matched ino FileType, update the new attribute values
            Some(attr) => {
                match atime {
                    Some(new_atime) => attr.atime = new_atime,
                    None => {}
                }
                match mtime {
                    Some(new_mtime) => attr.mtime = new_mtime,
                    None => {}
                }
                match crtime {
                    Some(new_crtime) => attr.crtime = new_crtime,
                    None => {}
                }
                match uid {
                    Some(new_uid) => {
                        attr.uid = new_uid;
                    }
                    None => {}
                }
                match gid {
                    Some(new_gid) => {
                        attr.gid = new_gid;
                    }
                    None => {}
                }
                match size {
                    Some(new_size) => {
                        if let Some(memfile) = self.files.get_mut(&ino) {
                            // First actually update the bytes in the file and then update the attr value
                            memfile.truncate_bytes(new_size);
                            attr.size = new_size;
                        }
                    }
                    None => {}
                }
                reply.attr(&Timespec::new(1,0), attr);
            }
            None => {
                error!("setattr: Cannot find inode: {}", ino);
                reply.error(ENOENT); // File not found error
            }
        }
    }

    /* This function reads all the files and directory in the current directory */
    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        let mut entries = Vec::new(); // Create an empty Vec to push all the files into this
        entries.push((ino, FileType::Directory, ".")); // pushing this directory first
        if let Some(inode) = self.inodes.get(&ino) {
            entries.push((inode.root, FileType::Directory, "..")); // pushing parent directory second
            for (child, child_ino) in &inode.nodes { // pushing all the children in this directory
                let child_attrs = &self.attrs.get(child_ino).unwrap();
                entries.push((child_attrs.ino, child_attrs.kind, &child));
            }
            if entries.len() > 0 {
                // Basically we need to add all the entries in ReplyAttr, but need to check if offset==0
                // otherwise the readdir() will be stuck in infinite loop.
                // When offset!=0, it means the passed offset has already been taken care off, and should skip it
                let to_skip = if offset == 0 { offset } else { offset + 1 } as usize;
                for (i, entry) in entries.into_iter().enumerate().skip(to_skip) {
                    reply.add(entry.0, i as i64, entry.1, entry.2);
                }
            }
            reply.ok();
        } 
        else {
            error!("readdir: cannot find inode: {}", ino);
            reply.error(ENOENT) // File not found error
        }
    }

    /* This function actually replies FileType based on the 'ino' number */
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        match self.inodes.get(&parent) {
            // First get the parent inode
            Some(parent_ino) => {
                let inode = match parent_ino.nodes.get(name.to_str().unwrap()) {
                    Some(inode) => inode, // Find if the inode is linked to parent or not
                    None => {
                        error!("lookup: {} is not in parent's {} children", name.to_str().unwrap(), parent);
                        reply.error(ENOENT);
                        return;
                    }
                };
                match self.attrs.get(inode) { // get the attributes of inode to send to reply
                    Some(attr) => {
                        reply.entry(&Timespec::new(1,0), attr, 0);
                    }
                    None => {
                        error!("lookup: cannot find inode: {}", inode);
                        reply.error(ENOENT); // File not found error
                    }
                };
            },
            // Parent inode not found
            None => {
                error!("lookup: parent inode: {} not found", parent);
                reply.error(ENOENT); // File not found error
            }
        };
    }

    /* This function removes a directory form the file-system */
    fn rmdir(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let mut rmdir_ino = 0;
        if let Some(parent_ino) = self.inodes.get_mut(&parent) { // first find the parent inode value
            match parent_ino.nodes.get(&name.to_str().unwrap().to_string()) { // then check if the FileType of 'name' exists
                Some(dir_ino) => {
                    rmdir_ino = *dir_ino;
                }
                // If not there, return error
                None => {
                    error!("rmdir: {} is not in parent's {} children", name.to_str().unwrap(), parent);
                    reply.error(ENOENT); // File not found error
                    return;
                }
            }
        }
        // Removing the FileType after searching if its there
        if let Some(dir) = self.inodes.get_mut(&rmdir_ino) {
            // Fist check if the directory is empty or not
            if dir.nodes.is_empty() {
                self.attrs.remove(&rmdir_ino);
            } 
            // else return error when removing
            else {
                error!("rmdir: failed to remove '{}': Directory not empty", dir.name);
                reply.error(ENOTEMPTY); // File is not empty error
                return;
            }
        }
        // If it's a file then remove it from the parent inode tree
        if let Some(parent_ino) = self.inodes.get_mut(&parent) {
            parent_ino.nodes.remove(&name.to_str().unwrap().to_string());
        }
        self.inodes.remove(&rmdir_ino);
        reply.ok();
    }

    /* This function is used to create directory in the file-system */
    fn mkdir(&mut self, _req: &Request, parent: u64, name: &OsStr, _mode: u32, reply: ReplyEntry) {
        let ts = time::now().to_timespec();
        let attr = FileAttr {
            ino: self.get_next_inode(), // get the next inode to add it under the parent
            size: 0,
            blocks: 0,
            atime: ts,
            mtime: ts,
            ctime: ts,
            crtime: ts,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
        };
        // Check if a parent exists or not
        if let Some(parent_ino) = self.inodes.get_mut(&parent) {
            // Check if the directory already exists or not
            if parent_ino.nodes.contains_key(name.to_str().unwrap()) {
                reply.error(EEXIST); // File exists error
                return;
            }
            // If not then just add a new dir to current parent inode
            parent_ino.nodes.insert(name.to_str().unwrap().to_string(), attr.ino);
            self.attrs.insert(attr.ino, attr);
        }
        else {
            error!("mkdir: cannot find parent {}", parent);
            reply.error(EINVAL); // Invalid argument error
            return;
        }
        // Create a new parent inode and then add it to existing tree
        self.inodes.insert(attr.ino, Inode::new_inode(name.to_str().unwrap().to_string(), parent));
        reply.entry(&Timespec::new(1,0), &attr, 0)
    }

    /* This function to open a file similar to 'touch' command */
    fn open(&mut self, _req: &Request, _ino: u64, _flags: u32, reply: ReplyOpen) {
        reply.opened(0, 0);
    }

    /* This function is remove a file from a parent directory */
    fn unlink(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let mut old_ino = 0; // variable to store previous inode value
        // first check if its not the root directory
        if let Some(parent_ino) = self.inodes.get_mut(&parent) {
            match parent_ino.nodes.remove(&name.to_str().unwrap().to_string()) { // check if the child is in parent or not
                Some(ino) => {
                    match self.attrs.remove(&ino) { // check 
                        Some(attr) => {
                            self.fs_size -= attr.size as i64;
                            if attr.kind == FileType::RegularFile{
                                self.files.remove(&ino); // if it's a file then remove it from FS
                            }
                            old_ino = ino; // and update the previous inode number with current inode
                        },
                        None => {
                            old_ino = ino;
                        },
                    }
                }
                None => {
                    error!("unlink: {} is not in parent's {} children", name.to_str().unwrap(), parent);
                    reply.error(ENOENT);
                    return;
                }
            }
        };
        self.inodes.remove(&old_ino); // This will remove the root directory inode if the old_ino is not updated
        reply.ok();
    }

    /* This function is used to create a file/dir in the file-system */
    fn create(&mut self, _req: &Request, parent: u64, name: &OsStr, _mode: u32, _flags: u32, reply: ReplyCreate) {
        let new_ino = self.get_next_inode(); // first get the next inode
        match self.inodes.get_mut(&parent) {
            Some(parent_ino) => {
                if let Some(ino) = parent_ino.nodes.get_mut(&name.to_str().unwrap().to_string()) { // if it exists then just update ReplyCreate, time of file and exit 
                    reply.created(&Timespec::new(1,0), self.attrs.get(&ino).unwrap(), 0, 0 ,0); 
                    return; // no need to throw an error
                }
                // just create a new file if not present then
                else {
                    let ts = time::now().to_timespec();
                    let attr = FileAttr {
                        ino: new_ino, // update it with the new inode value
                        size: 0,
                        blocks: 0,
                        atime: ts,
                        mtime: ts,
                        ctime: ts,
                        crtime: ts,
                        kind: FileType::RegularFile,
                        perm: 0o755,
                        nlink: 0,
                        uid: 0,
                        gid: 0,
                        rdev: 0,
                        flags: 0,
                    };
                    self.attrs.insert(attr.ino, attr); // Update file's attributes with it's respective inode value
                    self.files.insert(attr.ino, File::new_file()); // create a new file and add it to the FS
                    reply.created(&Timespec::new(1,0), &attr, 0, 0, 0); // update ReplyCreate with new timestamp
                }
                // insert current file-node with rest of the nodes
                parent_ino.nodes.insert(name.to_str().unwrap().to_string(), new_ino);
            }
            None => {
                error!("create: cannot find parent: {}", parent);
                reply.error(EINVAL); // Invalid argument error 
                return;
            }
        }
        // insert current inode value to the list of all the other inodes
        self.inodes.insert(new_ino, Inode::new_inode(name.to_str().unwrap().to_string(), parent));
    }

    /* This function is used to write something in a file */
    fn write(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, data: &[u8], _flags: u32, reply: ReplyWrite) {
        let ts = time::now().to_timespec(); // get the current time stamp
        //error!("ino: {}", ino);
        //error!("next_inode: {}", self.next_inode);
        match self.files.get_mut(&ino) { // find the file first
            Some(fp) => {
                match self.attrs.get_mut(&ino) { // get the file's attributes
                    Some(attr) => {                        

                        //println!("data.len(): {}", data.len());
                        if self.fs_size_spec as u64 > self.fs_max_size {
                            error!("Exceeding maximum allocated size for File-system!");
                            return;
                        }

                        let size = fp.update_file(offset, &data); // write the additional data to the file
                        attr.atime = ts; // update the timestamp
                        attr.mtime = ts;

                        attr.size = fp.get_file_size(); // update the new size to the file's attribute
                        if size !=4096 && !fp.is_updated {
                            self.fs_size += fp.get_file_size() as i64;
                            fp.is_updated = true;
                            fp.old_size = fp.get_file_size() as i64;
                        }
                        else if size != 4096 {
                            self.fs_size += (fp.get_file_size() as i64) - fp.old_size;
                            fp.old_size = fp.get_file_size() as i64;
                        }
                        println!("fs size: {}", self.fs_size);
                        reply.written(size as u32);
                        
                        /***
                        println!("After: fs size: {}", self.fs_size);
                        if self.fs_size as u64 > self.fs_max_size {
                            error!("Exceeding maximum allocated size for File-system!");
                            reply.error(EFBIG);
                            return;
                        }
                        ***/
                        
                    }
                    None => {
                        error!("write: cannot find ino: {}", ino);
                        reply.error(ENOENT); // No such file or directory error
                    }
                }
            }
            // if file doesn't exist then throw error
            None => reply.error(ENOENT), // No such file or directory error
        }
    }

    /* This functions is there to read a file */
    fn read(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, _size: u32, reply: ReplyData) {  
        // similar to write(), but there is no updation only writing to the ReplyData 
        match self.files.get_mut(&ino) {
            Some(fp) => {
                let mut thread_attrs = self.attrs.clone();
                let thread_fp = fp.clone();
                match thread_attrs.get_mut(&ino) {
                    Some(attr) => {
                        attr.atime = time::now().to_timespec();
                        reply.data(&thread_fp.data[offset as usize..]);
                    },
                    None => {
                        error!("read: cannot find ino: {}", ino);
                        reply.error(ENOENT); // No such file or directory error
                    },
                }
            }
            None => {
                reply.error(ENOENT); // No such file or directory error
            }
        }
    }

    /* This function is to rename a file or directory in the FS */
    fn rename(&mut self, _req: &Request, parent: u64, name: &OsStr, newparent: u64, newname: &OsStr, reply: ReplyEmpty) {
        // First find the file with the passed 'ino' value
        if self.inodes.contains_key(&parent) && self.inodes.contains_key(&newparent) {
            let file_ino;
            match self.inodes.get_mut(&parent) {
                Some(parent_ino) => { // remove the older version of the file and its existence
                    if let Some(ino) = parent_ino.nodes.remove(&name.to_str().unwrap().to_string()) {
                        file_ino = ino;
                    } 
                    else {
                        error!("{} not found in parent {}", name.to_str().unwrap().to_string(), parent);
                        reply.error(ENOENT); // No such file or directory error
                        return;
                    }
                }
                None => {
                    error!("rename: cannot find parent: {}", parent);
                    reply.error(EINVAL);
                    return;
                }
            }
            // Update the new file name and its new inode value
            if let Some(newparent_ino) = self.inodes.get_mut(&newparent) {
                newparent_ino.nodes.insert(newname.to_str().unwrap().to_string(), file_ino);
            }
        }
        reply.ok(); // reply to a request with nothing
    }
}
