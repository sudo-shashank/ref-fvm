// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::marker::PhantomData;

use serde::de::{self, Deserialize};
use serde::ser::{self, Serialize};

use crate::node::CollapsedNode;
use crate::{init_sized_vec, Node};

#[derive(Debug, PartialEq)]
pub struct VersionV0;
#[derive(Debug, PartialEq)]
pub struct VersionV3;

/// Root of an AMT vector, can be serialized and keeps track of height and count
#[derive(PartialEq, Debug)]
pub struct Root<V, Version = VersionV3> {
    pub bit_width: u32,
    pub height: u32,
    pub count: u64,
    pub node: Node<V>,
    version: PhantomData<Version>
}

impl<V, Version> Root<V, Version> {
    pub(super) fn new(bit_width: u32) -> Self {
        Self {
            bit_width,
            count: 0,
            height: 0,
            node: Node::Leaf {
                vals: init_sized_vec(bit_width),
            },
            version: PhantomData
        }
    }
}

impl<V> Serialize for Root<V, VersionV0>
where
    V: Serialize,
{
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        (&self.height, &self.count, &self.node).serialize(s)
    }
}

impl<'de, V> Deserialize<'de> for Root<V, VersionV0>
where
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let (height, count, node): ( _, _, CollapsedNode<V>) =
            Deserialize::deserialize(deserializer)?;
        Ok(Self {
            bit_width: crate::DEFAULT_BIT_WIDTH,
            height,
            count,
            node: node.expand(crate::DEFAULT_BIT_WIDTH).map_err(de::Error::custom)?,
            version: PhantomData
        })
    }
}

impl<V> Serialize for Root<V>
where
    V: Serialize,
{
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        (&self.bit_width, &self.height, &self.count, &self.node).serialize(s)
    }
}

impl<'de, V> Deserialize<'de> for Root<V>
where
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let (bit_width, height, count, node): (_, _, _, CollapsedNode<V>) =
            Deserialize::deserialize(deserializer)?;
        Ok(Self {
            bit_width,
            height,
            count,
            node: node.expand(bit_width).map_err(de::Error::custom)?,
            version: PhantomData
        })
    }
}

#[cfg(test)]
mod tests {
    use fvm_ipld_encoding::{from_slice, to_vec};

    use super::*;

    #[test]
    fn serialize_symmetric() {
        let mut root = Root::new(0);
        root.height = 2;
        root.count = 1;
        root.node = Node::Leaf { vals: vec![None] };
        let rbz = to_vec(&root).unwrap();
        assert_eq!(from_slice::<Root<String>>(&rbz).unwrap(), root);
    }
}
