use fvm_ipld_encoding::CodecProtocol::Cbor;
// TODO: We'll probably need our own error type here
use fvm_ipld_encoding::Error;
use fvm_ipld_encoding::DAG_CBOR;
use {serde, serde_ipld_dagcbor};

// TODO: Slapped the Serialize derivations on for some actors testing, not clear to me it should stay
#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Default)]
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
            // IPLD_RAW => BytesDeserializer::new(self.data.as_slice())
            //     .deser()
            //     .map_err(Into::into),
            DAG_CBOR => serde_ipld_dagcbor::from_slice(self.data.as_slice()).map_err(Into::into),
            _ => Err(Error {
                description: "unsupported protocol".to_string(),
                protocol: Cbor,
            }),
        }
    }
    pub fn serialize<T: serde::Serialize + ?Sized>(codec: u64, value: &T) -> Result<Self, Error> {
        let data = match codec {
            DAG_CBOR => serde_ipld_dagcbor::to_vec(value)?,
            _ => {
                return Err(Error {
                    description: "unsupported protocol".to_string(),
                    protocol: Cbor,
                });
            }
        };
        Ok(IpldBlock { codec, data })
    }
    pub fn serialize_cbor<T: serde::Serialize + ?Sized>(value: &T) -> Result<Self, Error> {
        IpldBlock::serialize(DAG_CBOR, value)
    }
}
