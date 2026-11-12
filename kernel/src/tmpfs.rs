//! Bounded in-memory filesystem primitive (tmpfs).
use alloc::{collections::BTreeMap, string::String, vec::Vec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TmpFsError { NotFound, NotDirectory, IsDirectory, AlreadyExists, NotEmpty, InvalidPath, NoSpace, NameTooLong }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TmpFsStat { pub is_directory: bool, pub size: usize }

enum NodeKind { Directory(BTreeMap<String, Node>), File(Vec<u8>) }
struct Node { kind: NodeKind }
impl Node {
    fn directory() -> Self { Self { kind: NodeKind::Directory(BTreeMap::new()) } }
    fn file() -> Self { Self { kind: NodeKind::File(Vec::new()) } }
}

/// In-memory hierarchical filesystem with a hard byte budget for file payloads.
pub struct TmpFs { root: Node, byte_limit: usize, used_bytes: usize }
impl TmpFs {
    pub fn new(byte_limit: usize) -> Self {
        Self { root: Node::directory(), byte_limit, used_bytes: 0 }
    }
    pub fn used_bytes(&self) -> usize { self.used_bytes }
    pub fn byte_limit(&self) -> usize { self.byte_limit }

    fn components(path: &str) -> Result<Vec<&str>, TmpFsError> {
        if !path.starts_with('/') || path.as_bytes().contains(&0) { return Err(TmpFsError::InvalidPath); }
        if path == "/" { return Ok(Vec::new()); }
        let parts: Vec<&str> = path[1..].split('/').collect();
        if parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.len() > 255) {
            return Err(if parts.iter().any(|p| p.len() > 255) { TmpFsError::NameTooLong } else { TmpFsError::InvalidPath });
        }
        Ok(parts)
    }
    fn node<'a>(mut node: &'a Node, parts: &[&str]) -> Result<&'a Node, TmpFsError> {
        for part in parts {
            match &node.kind {
                NodeKind::Directory(entries) => node = entries.get(*part).ok_or(TmpFsError::NotFound)?,
                NodeKind::File(_) => return Err(TmpFsError::NotDirectory),
            }
        }
        Ok(node)
    }
    fn parent_mut<'a>(mut node: &'a mut Node, parts: &[&str]) -> Result<(&'a mut Node, String), TmpFsError> {
        let (name, parents) = parts.split_last().ok_or(TmpFsError::InvalidPath)?;
        for part in parents {
            match &mut node.kind {
                NodeKind::Directory(entries) => node = entries.get_mut(*part).ok_or(TmpFsError::NotFound)?,
                NodeKind::File(_) => return Err(TmpFsError::NotDirectory),
            }
        }
        Ok((node, String::from(*name)))
    }
    pub fn mkdir(&mut self, path: &str) -> Result<(), TmpFsError> {
        let parts=Self::components(path)?;
        let (parent,name)=Self::parent_mut(&mut self.root,&parts)?;
        match &mut parent.kind {
            NodeKind::Directory(entries) => {
                if entries.contains_key(&name) { return Err(TmpFsError::AlreadyExists); }
                entries.insert(name,Node::directory()); Ok(())
            }
            NodeKind::File(_) => Err(TmpFsError::NotDirectory),
        }
    }
    pub fn create(&mut self, path: &str) -> Result<(), TmpFsError> {
        let parts=Self::components(path)?;
        let (parent,name)=Self::parent_mut(&mut self.root,&parts)?;
        match &mut parent.kind {
            NodeKind::Directory(entries) => {
                if entries.contains_key(&name) { return Err(TmpFsError::AlreadyExists); }
                entries.insert(name,Node::file()); Ok(())
            }
            NodeKind::File(_) => Err(TmpFsError::NotDirectory),
        }
    }
    pub fn stat(&self,path:&str)->Result<TmpFsStat,TmpFsError>{
        let parts=Self::components(path)?;
        let node=Self::node(&self.root,&parts)?;
        match &node.kind {
            NodeKind::Directory(entries)=>Ok(TmpFsStat{is_directory:true,size:entries.len()}),
            NodeKind::File(data)=>Ok(TmpFsStat{is_directory:false,size:data.len()}),
        }
    }
    pub fn read(&self,path:&str,offset:usize,out:&mut [u8])->Result<usize,TmpFsError>{
        let parts=Self::components(path)?;
        match &Self::node(&self.root,&parts)?.kind {
            NodeKind::Directory(_)=>Err(TmpFsError::IsDirectory),
            NodeKind::File(data)=>{
                if offset>=data.len(){return Ok(0);}
                let count=out.len().min(data.len()-offset);
                out[..count].copy_from_slice(&data[offset..offset+count]); Ok(count)
            }
        }
    }
    pub fn write(&mut self,path:&str,offset:usize,input:&[u8])->Result<usize,TmpFsError>{
        let parts=Self::components(path)?;
        let (parent,name)=Self::parent_mut(&mut self.root,&parts)?;
        let data=match &mut parent.kind {
            NodeKind::Directory(entries)=>match &mut entries.get_mut(&name).ok_or(TmpFsError::NotFound)?.kind {
                NodeKind::File(data)=>data, NodeKind::Directory(_)=>return Err(TmpFsError::IsDirectory),
            },
            NodeKind::File(_)=>return Err(TmpFsError::NotDirectory),
        };
        let end=offset.checked_add(input.len()).ok_or(TmpFsError::NoSpace)?;
        let new_len=data.len().max(end);
        let growth=new_len-data.len();
        if self.used_bytes.checked_add(growth).is_none_or(|n|n>self.byte_limit){return Err(TmpFsError::NoSpace);}
        if growth>0 { data.resize(new_len,0); self.used_bytes+=growth; }
        data[offset..end].copy_from_slice(input);
        Ok(input.len())
    }
    pub fn truncate(&mut self,path:&str,new_len:usize)->Result<(),TmpFsError>{
        let parts=Self::components(path)?;
        let (parent,name)=Self::parent_mut(&mut self.root,&parts)?;
        let data=match &mut parent.kind {
            NodeKind::Directory(entries)=>match &mut entries.get_mut(&name).ok_or(TmpFsError::NotFound)?.kind {
                NodeKind::File(data)=>data,NodeKind::Directory(_)=>return Err(TmpFsError::IsDirectory),
            },
            NodeKind::File(_)=>return Err(TmpFsError::NotDirectory),
        };
        if new_len>data.len() {
            let growth=new_len-data.len();
            if self.used_bytes.checked_add(growth).is_none_or(|n|n>self.byte_limit){return Err(TmpFsError::NoSpace);}
            data.resize(new_len,0); self.used_bytes+=growth;
        } else {
            self.used_bytes-=data.len()-new_len; data.truncate(new_len);
        }
        Ok(())
    }
    pub fn unlink(&mut self,path:&str)->Result<(),TmpFsError>{
        let parts=Self::components(path)?;
        let (parent,name)=Self::parent_mut(&mut self.root,&parts)?;
        let removed=match &mut parent.kind {
            NodeKind::Directory(entries)=>entries.remove(&name).ok_or(TmpFsError::NotFound)?,
            NodeKind::File(_)=>return Err(TmpFsError::NotDirectory),
        };
        match removed.kind {
            NodeKind::File(data)=>self.used_bytes-=data.len(),
            NodeKind::Directory(entries)=>{
                if !entries.is_empty() {
                    let (parent,name)=Self::parent_mut(&mut self.root,&parts)?;
                    if let NodeKind::Directory(parent_entries)=&mut parent.kind { parent_entries.insert(name,Node{kind:NodeKind::Directory(entries)}); }
                    return Err(TmpFsError::NotEmpty);
                }
            }
        }
        Ok(())
    }
    pub fn rename(&mut self,from:&str,to:&str)->Result<(),TmpFsError>{
        let src=Self::components(from)?;
        let dst=Self::components(to)?;
        if src.is_empty()||dst.is_empty(){return Err(TmpFsError::InvalidPath);}
        if dst.starts_with(&src){return Err(TmpFsError::InvalidPath);}
        let (src_parent,src_name)=Self::parent_mut(&mut self.root,&src)?;
        let node=match &mut src_parent.kind {NodeKind::Directory(e)=>e.remove(&src_name).ok_or(TmpFsError::NotFound)?,NodeKind::File(_)=>return Err(TmpFsError::NotDirectory)};
        let inserted={
            let (dst_parent,dst_name)=Self::parent_mut(&mut self.root,&dst)?;
            match &mut dst_parent.kind {
                NodeKind::Directory(e)=>{if e.contains_key(&dst_name){false}else{e.insert(dst_name,node);true}},
                NodeKind::File(_)=>false,
            }
        };
        if inserted {Ok(())} else {
            let (src_parent,src_name)=Self::parent_mut(&mut self.root,&src)?;
            if let NodeKind::Directory(e)=&mut src_parent.kind {e.insert(src_name,node);}
            Err(TmpFsError::AlreadyExists)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn nested_create_write_read_and_stat(){
        let mut fs=TmpFs::new(128);fs.mkdir("/tmp").unwrap();fs.create("/tmp/a").unwrap();
        assert_eq!(fs.write("/tmp/a",2,b"data").unwrap(),4);
        let mut out=[0;8];assert_eq!(fs.read("/tmp/a",0,&mut out).unwrap(),6);
        assert_eq!(&out[..6],b"\0\0data");assert_eq!(fs.stat("/tmp/a").unwrap().size,6);
    }
    #[test] fn truncate_and_unlink_release_budget(){
        let mut fs=TmpFs::new(4);fs.create("/x").unwrap();fs.write("/x",0,b"abcd").unwrap();
        assert_eq!(fs.write("/x",4,b"e"),Err(TmpFsError::NoSpace));
        fs.truncate("/x",2).unwrap();assert_eq!(fs.used_bytes(),2);
        fs.unlink("/x").unwrap();assert_eq!(fs.used_bytes(),0);
    }
    #[test] fn rename_is_atomic_on_existing_destination(){
        let mut fs=TmpFs::new(32);fs.create("/a").unwrap();fs.create("/b").unwrap();
        assert_eq!(fs.rename("/a","/b"),Err(TmpFsError::AlreadyExists));
        assert!(fs.stat("/a").is_ok());assert!(fs.stat("/b").is_ok());
    }
    #[test] fn nonempty_directory_cannot_be_removed(){
        let mut fs=TmpFs::new(32);fs.mkdir("/d").unwrap();fs.create("/d/f").unwrap();
        assert_eq!(fs.unlink("/d"),Err(TmpFsError::NotEmpty));
        assert!(fs.stat("/d/f").is_ok());
    }
}
