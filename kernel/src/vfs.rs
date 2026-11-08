//! Minimal Virtual File System (VFS) for user-space files.

use core::fmt;

/// Identificateur de type de fichier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileType {
    Directory,
    File,
    CharacterDevice,
    BlockDevice,
}

/// Entrée de répertoire.
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: &'static str,
    pub file_type: FileType,
}

/// Inode virtuel (placeholder pour T14).
#[derive(Clone, Copy, Debug, Default)]
pub struct Inode {
    pub number: u64,
    pub size: u64,
    pub flags: u32,
}

/// Drapeaux d'ouverture de fichier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OpenFlags {
    /// Lecture seule.
    ReadOnly = 1,
    /// Écriture seule.
    WriteOnly = 2,
    /// Lecture et écriture.
    ReadWrite = 3,
    /// Créer le fichier s'il n'existe pas.
    Create = 4,
    /// Tronquer le fichier existant.
    Truncate = 8,
}

/// Superblock virtuel (état global du VFS).
#[derive(Clone, Copy, Debug, Default)]
pub struct Superblock {
    pub magic: u32,
    pub root_inode: Inode,
}

/// Erreurs VFS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VfsError {
    NotFound,
    AlreadyExists,
    PermissionDenied,
    InvalidPath,
    OutOfSpace,
    UnsupportedOperation,
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VfsError::NotFound => write!(f, "fichier introuvable"),
            VfsError::AlreadyExists => write!(f, "fichier déjà existant"),
            VfsError::PermissionDenied => write!(f, "permission refusée"),
            VfsError::InvalidPath => write!(f, "chemin invalide"),
            VfsError::OutOfSpace => write!(f, "espace insuffisant"),
            VfsError::UnsupportedOperation => write!(f, "opération non supportée"),
        }
    }
}

/// Descripteur de fichier ouvert.
#[derive(Clone, Copy, Debug)]
pub struct File {
    pub inode: Inode,
    pub flags: OpenFlags,
    pub position: u64,
}

/// Monte un système de fichiers racine virtuel.
///
/// Pour l'instant, retourne un superblock par défaut racine sur `/`.
pub fn mount_root() -> Result<Superblock, VfsError> {
    Ok(Superblock {
        magic: 0x12345678,
        root_inode: Inode {
            number: 1,
            size: 0,
            flags: 0,
        },
    })
}

/// Ouvre un fichier par chemin (implémentation minimale).
pub fn open(path: &str, flags: OpenFlags) -> Result<File, VfsError> {
    // Surcharge simplifiée : on ne fait que valider le chemin
    if path.is_empty() {
        return Err(VfsError::InvalidPath);
    }
    if path == "/" {
        return Ok(File {
            inode: Inode {
                number: 1,
                size: 0,
                flags: 0,
            },
            flags,
            position: 0,
        });
    }
    // TODO : implémenter le vrai système de fichiers T14
    Err(VfsError::NotFound)
}

/// Lit depuis un descripteur de fichier.
pub fn read(_file: &mut File, _buf: &mut [u8]) -> Result<usize, VfsError> {
    // TODO : implémenter la lecture réelle T14
    Err(VfsError::UnsupportedOperation)
}

/// Écrit dans un descripteur de fichier.
pub fn write(_file: &mut File, _buf: &[u8]) -> Result<usize, VfsError> {
    // TODO : implémenter l'écriture réelle T14
    Err(VfsError::UnsupportedOperation)
}

/// Ferme un descripteur de fichier.
pub fn close(_file: &mut File) -> Result<(), VfsError> {
    // TODO : implémenter la fermeture réelle T14
    Ok(())
}

/// Itère sur les entrées de répertoire (implémentation minimale).
pub fn read_dir(_dir: &str) -> Result<&'static [DirEntry], VfsError> {
    // TODO : implémenter la lecture de répertoire T14
    // Return static empty slice
    static EMPTY: [DirEntry; 0] = [];
    Ok(&EMPTY)
}

/// Vérifie si un chemin est absolu.
pub fn is_absolute(path: &str) -> bool {
    path.starts_with('/')
}

/// Vérifie si un chemin est racine.
pub fn is_root(path: &str) -> bool {
    path == "/"
}