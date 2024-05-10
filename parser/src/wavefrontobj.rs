use anyhow::{anyhow, bail, Result};
use std::io::BufRead;
use std::io::{BufReader, Read};
use std::str::FromStr;

#[derive(Debug, Default, Clone)]
struct Model {
    pub v: Vec<Vec<f32>>,
    pub vn: Vec<Vec<f32>>,
    pub vt: Vec<Vec<f32>>,
    pub f: Vec<Vec<Vec<i64>>>,
}
#[allow(dead_code)]
impl Model {
    const POINT: usize = 0;
    const NORMAL: usize = 1;
    const TEXTURE: usize = 2;
}

impl Model {
    ///return FlattenVertex
    ///\[ point{x,y,z}, normal{x,y,z}, uv{u,v}, ...\]
    pub fn to_vertex(&self) -> Option<Vec<f32>> {
        let mut vertex = Vec::with_capacity(self.f.len() * 9);
        for f in self.f.iter() {
            for point in f.iter() {
                if let &[p, t, n] = point.as_slice() {
                    //tとnの入れ替えを含む
                    vertex.extend(i_get(&self.v, p).unwrap());
                    vertex.extend(i_get(&self.vn, n).unwrap());
                    vertex.extend(i_get(&self.vt, t).unwrap());
                }
            }
        }
        Some(vertex)
    }
}
/// Pythonぽい負値インデックスによるアクセス
fn i_get<T>(v: &Vec<T>, index: i64) -> Option<&T> {
    match index {
        1.. => v.get(index as usize - 1),
        0 => unreachable!(),
        _ => v.get((v.len() as i64 + index) as usize),
    }
}

/// 複数のstrを一気にパース
fn parse<'a, T, I>(i: I) -> Result<Vec<T>, <T as FromStr>::Err>
where
    T: FromStr,
    I: IntoIterator<Item = &'a str>,
{
    i.into_iter().map(|i| i.parse::<T>()).collect()
}

pub fn parse_obj(buf: impl Read) -> Result<(String, Vec<f32>)> {
    let mut f = BufReader::new(buf);
    // let mut ms = HashMap::new();
    let m = &mut Model::default();
    let mut mtl = String::new();
    let mut buf = String::new();

    loop {
        buf.clear();
        if let Err(_) = f.read_line(&mut buf) {
            break;
        }
        if buf.is_empty() {
            break;
        }
        let split = buf
            .split("#")
            .next()
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        if split.is_empty() {
            continue;
        }
        match split.as_slice() {
            &["mtllib", lib, ..] => mtl = lib.to_owned(),
            &["usemtl", ..] => (),
            &["v", x, y, z, ..] => m.v.push(parse([x, y, z])?),
            &["vt", u, v, ..] => m.vt.push(parse([u, v])?),
            &["vn", x, y, z] => m.vn.push(parse([x, y, z])?),
            &["f", x, y, z, ..] => m.f.push(vec![
                parse(x.split("/"))?,
                parse(y.split("/"))?,
                parse(z.split("/"))?,
            ]),
            &["o", ..] => (),
            &["s", ..] => (),
            _ => bail!("invalide input: \n\"{}\"\nparsed: {:?}", buf, split),
        }
    }
    Ok((mtl, m.to_vertex().ok_or(anyhow!("invalid point index"))?))
}
