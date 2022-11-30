use serde::de::value;
use {serde, serde_ipld_dagcbor};

use crate::codec::{DAG_CBOR, IPLD_RAW};
use crate::{CodecProtocol, Error, RawBytes};

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct IpldBlock {
    pub codec: u64,
    pub data: Vec<u8>,
}

impl IpldBlock {
    pub fn deserialize<'de, T>(&'de self) -> Result<T, Error>
    where
        T: serde::Deserialize<'de>,
    {
        match self.codec {
            IPLD_RAW => T::deserialize(value::BytesDeserializer::<value::Error>::new(
                self.data.as_slice(),
            ))
            .map_err(|e| Error {
                description: e.to_string(),
                protocol: CodecProtocol::Raw,
            }),
            DAG_CBOR => Ok(serde_ipld_dagcbor::from_slice(self.data.as_slice())?),
            _ => Err(Error {
                description: "unsupported protocol".to_string(),
                protocol: CodecProtocol::Unsupported,
            }),
        }
    }
    pub fn serialize<T: serde::Serialize + ?Sized>(codec: u64, value: &T) -> Result<Self, Error> {
        let data = match codec {
            // TODO: Steb will do things
            // IPLD_RAW: BytesS
            DAG_CBOR => serde_ipld_dagcbor::to_vec(value)?,
            _ => {
                return Err(Error {
                    description: "unsupported protocol".to_string(),
                    protocol: CodecProtocol::Unsupported,
                });
            }
        };
        Ok(IpldBlock { codec, data })
    }
    pub fn serialize_cbor<T: serde::Serialize + ?Sized>(value: &T) -> Result<Self, Error> {
        IpldBlock::serialize(DAG_CBOR, value)
    }
}

impl From<RawBytes> for Option<IpldBlock> {
    fn from(other: RawBytes) -> Self {
        (!other.is_empty()).then(|| IpldBlock {
            codec: DAG_CBOR,
            data: other.into(),
        })
    }
}
