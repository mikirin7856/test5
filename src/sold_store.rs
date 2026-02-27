use std::sync::{Arc, Mutex};

use anyhow::Result;
use rocksdb::{DB, Options, WriteBatch};
use tokio::task;
use xxhash_rust::xxh3::xxh3_128;

#[derive(Clone)]
pub struct SoldStore {
    db: Arc<DB>,
    claim_lock: Arc<Mutex<()>>,
}

#[derive(Clone, Debug)]
pub struct SoldCandidate {
    pub main_domain: String,
    pub login: String,
    pub password: String,
}

impl SoldStore {
    pub async fn new(path: &str) -> Result<Self> {
        let path = path.to_string();

        let db = task::spawn_blocking(move || -> Result<DB> {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.increase_parallelism(4);
            opts.optimize_level_style_compaction(512 * 1024 * 1024);
            Ok(DB::open(&opts, path)?)
        })
        .await??;

        Ok(Self {
            db: Arc::new(db),
            claim_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn make_key(main_domain: &str, login: &str, password: &str) -> [u8; 32] {
        let mut s = String::with_capacity(main_domain.len() + login.len() + password.len() + 2);
        s.push_str(main_domain);
        s.push('\0');
        s.push_str(login);
        s.push('\0');
        s.push_str(password);
        let h: u128 = xxh3_128(s.as_bytes());
        let mut out = [0u8; 32];
        write_u128_hex32(h, &mut out);
        out
    }

    pub async fn contains(&self, main_domain: &str, login: &str, password: &str) -> Result<bool> {
        let db = self.db.clone();
        let key = Self::make_key(main_domain, login, password);

        task::spawn_blocking(move || -> Result<bool> { Ok(db.get(key)?.is_some()) }).await?
    }

    pub async fn filter_existing_batch(&self, keys: Vec<[u8; 32]>) -> Result<Vec<bool>> {
        let db = self.db.clone();

        task::spawn_blocking(move || -> Result<Vec<bool>> {
            let refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
            let res = db.multi_get(refs);

            let mut out = Vec::with_capacity(res.len());
            for item in res {
                out.push(item?.is_some());
            }
            Ok(out)
        })
        .await?
    }

    pub async fn mark_sold_batch(
        &self,
        main_domain: &str,
        pairs: &[(String, String)],
    ) -> Result<()> {
        let db = self.db.clone();
        let domain = main_domain.to_string();
        let pairs = pairs.to_vec();

        task::spawn_blocking(move || -> Result<()> {
            let mut batch = WriteBatch::default();
            for (login, pass) in pairs {
                let key = SoldStore::make_key(&domain, &login, &pass);
                batch.put(key, b"1");
            }
            db.write(batch)?;
            Ok(())
        })
        .await?
    }

    pub async fn claim_unsold(
        &self,
        candidates: Vec<SoldCandidate>,
        requested: usize,
    ) -> Result<Vec<SoldCandidate>> {
        let db = self.db.clone();
        let claim_lock = self.claim_lock.clone();

        task::spawn_blocking(move || -> Result<Vec<SoldCandidate>> {
            let _guard = claim_lock.lock().expect("claim lock poisoned");
            let mut selected = Vec::with_capacity(requested);
            let mut batch = WriteBatch::default();

            for candidate in candidates {
                if selected.len() == requested {
                    break;
                }

                let key = SoldStore::make_key(
                    &candidate.main_domain,
                    &candidate.login,
                    &candidate.password,
                );
                if db.get(key)?.is_none() {
                    batch.put(key, b"1");
                    selected.push(candidate);
                }
            }

            if !selected.is_empty() {
                db.write(batch)?;
            }

            Ok(selected)
        })
        .await?
    }
}

fn write_u128_hex32(x: u128, out: &mut [u8; 32]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for i in 0..32 {
        let shift = 4 * (31 - i);
        let nibble = ((x >> shift) & 0xF) as usize;
        out[i] = HEX[nibble];
    }
}
