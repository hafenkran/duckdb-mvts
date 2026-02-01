use duckdb::core::{DataChunkHandle, Inserter, LogicalTypeId};
use duckdb::vscalar::{ScalarFunctionSignature, VScalar};
use duckdb::vtab::arrow::WritableVector;

use super::{server_status, start_server, stop_server};

pub struct StartServerScalar;

impl VScalar for StartServerScalar {
    type State = ();

    unsafe fn invoke(
        _: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let port_vector = input.flat_vector(0);
        let ports = port_vector.as_slice_with_len::<i32>(input.len());
        let out = output.flat_vector();

        for port in ports.iter().take(input.len()) {
            let port_value = *port;
            if port_value <= 0 || port_value > u16::MAX as i32 {
                return Err(format!("Invalid port: {port_value}").into());
            }
            let msg = start_server(port_value as u16)?;
            out.insert(0, msg.as_str());
        }
        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![LogicalTypeId::Integer.into()],
            LogicalTypeId::Varchar.into(),
        )]
    }
}

pub struct StopServerScalar;

impl VScalar for StopServerScalar {
    type State = ();

    unsafe fn invoke(
        _: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let out = output.flat_vector();
        let count = input.len().max(1);
        for _ in 0..count {
            let msg = stop_server()?;
            out.insert(0, msg.as_str());
        }
        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![],
            LogicalTypeId::Varchar.into(),
        )]
    }
}

pub struct ServerStatusScalar;

impl VScalar for ServerStatusScalar {
    type State = ();

    unsafe fn invoke(
        _: &Self::State,
        input: &mut DataChunkHandle,
        output: &mut dyn WritableVector,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let out = output.flat_vector();
        let count = input.len().max(1);
        for _ in 0..count {
            let msg = server_status();
            out.insert(0, msg.as_str());
        }
        Ok(())
    }

    fn signatures() -> Vec<ScalarFunctionSignature> {
        vec![ScalarFunctionSignature::exact(
            vec![],
            LogicalTypeId::Varchar.into(),
        )]
    }
}
