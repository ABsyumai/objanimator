use asyncfileio::FileConverter;
use parser::wavefrontobj::parse_obj;
use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let src = std::env::args().nth(1).unwrap();
    FileConverter::spawn(vec![src], |s, buf| {
        let (_, b) = parse_obj(buf.as_ref().as_slice())?;
        Ok((PathBuf::from(s).with_extension("vertex"), b))
    })
    .stop()?;
    Ok(())
}
