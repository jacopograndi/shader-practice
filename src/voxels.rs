use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use bytemuck::{Pod, Zeroable};
use glam::IVec3;

pub const CHUNK_SIDE: usize = 32;
pub const CHUNK_AREA: usize = CHUNK_SIDE * CHUNK_SIDE;
pub const CHUNK_VOLUME: usize = CHUNK_AREA * CHUNK_SIDE;

pub fn simple_universe() -> Universe {
    let mut chunks = HashMap::new();
    {
        // sphere at 16,16,16
        let chunk = Chunk::empty();
        for xyz in Chunk::iter() {
            if xyz.distance_squared(IVec3::splat((CHUNK_SIDE as i32) / 2)) < 16 * 16 {
                let id = xyz.x as u8;
                chunk.set_block(xyz, Block::from_id(id));
            } else {
                chunk.set_block(xyz, Block::from_id(0));
            }
        }
        chunks.insert(IVec3::new(0, 0, 0), chunk);
    }
    Universe { chunks }
}

#[derive(Debug, Clone, Default)]
pub struct Universe {
    pub chunks: HashMap<IVec3, Chunk>,
}

impl Universe {
    fn pos_to_chunk_and_inner(&self, pos: &IVec3) -> (IVec3, IVec3) {
        let chunk_size = IVec3::splat(CHUNK_SIDE as i32);
        let chunk_pos = (pos.div_euclid(chunk_size)) * chunk_size;
        let inner_pos = pos.rem_euclid(chunk_size);
        (chunk_pos, inner_pos)
    }

    pub fn read_chunk_block(&self, pos: &IVec3) -> Option<Block> {
        let (chunk_pos, inner_pos) = self.pos_to_chunk_and_inner(pos);
        self.chunks
            .get(&chunk_pos)
            .map(|chunk| chunk.read_block(inner_pos))
    }

    pub fn set_chunk_block(&mut self, pos: &IVec3, block: Block) {
        let (chunk_pos, inner_pos) = self.pos_to_chunk_and_inner(pos);
        if let Some(chunk) = self.chunks.get_mut(&chunk_pos) {
            chunk.set_block(inner_pos, block);
            chunk.dirty_render = true;
        } else {
            let mut chunk = Chunk::empty();
            chunk.set_block(inner_pos, block);
            chunk.dirty_render = true;
            self.chunks.insert(chunk_pos, chunk);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Chunk {
    _blocks: Arc<RwLock<[Block; CHUNK_VOLUME]>>,
    pub dirty_render: bool,
}

impl Chunk {
    pub fn iter() -> impl Iterator<Item = IVec3> {
        (0..CHUNK_VOLUME).map(Self::idx2xyz)
    }

    pub fn get_ref(&self) -> RwLockReadGuard<[Block; CHUNK_VOLUME]> {
        self._blocks.read().unwrap()
    }

    pub fn get_mut(&self) -> RwLockWriteGuard<[Block; CHUNK_VOLUME]> {
        self._blocks.write().unwrap()
    }

    pub fn empty() -> Self {
        Self {
            _blocks: Arc::new(RwLock::new([Block::default(); CHUNK_VOLUME])),
            dirty_render: false,
        }
    }

    pub fn filled(id: u8) -> Self {
        let block = Block::from_id(id);
        Self {
            _blocks: Arc::new(RwLock::new([block; CHUNK_VOLUME])),
            dirty_render: false,
        }
    }

    pub fn set_block(&self, xyz: IVec3, block: Block) {
        self._blocks.write().unwrap()[Self::xyz2idx(xyz)] = block;
    }

    pub fn read_block(&self, xyz: IVec3) -> Block {
        self._blocks.read().unwrap()[Self::xyz2idx(xyz)]
    }

    pub fn xyz2idx(xyz: IVec3) -> usize {
        xyz.x as usize * CHUNK_AREA + xyz.y as usize * CHUNK_SIDE + xyz.z as usize
    }

    pub fn idx2xyz(index: usize) -> IVec3 {
        let layer = index / CHUNK_SIDE;
        IVec3 {
            x: (layer / CHUNK_SIDE) as i32,
            y: (layer % CHUNK_SIDE) as i32,
            z: (index % CHUNK_SIDE) as i32,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Pod, Zeroable, Copy, Default, PartialEq, Eq)]
pub struct Block {
    pub id: u8,
    pub properties: u8,
    pub light0: u8,
    pub light1: u8,
}

impl Block {
    fn from_id(id: u8) -> Block {
        Self {
            id,
            properties: 0,
            light0: 0,
            light1: 0,
        }
    }
}
