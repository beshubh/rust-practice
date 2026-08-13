fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use std::{
        num::IntErrorKind::NegOverflow,
        sync::atomic::{
            AtomicU32, AtomicUsize,
            Ordering::{Relaxed, SeqCst},
        },
        time::Duration,
    };

    #[test]
    fn test_fetch_add() {
        let process_item = |tid, ms| {
            println!("thread: {tid}, doing work");
            std::thread::sleep(std::time::Duration::from_millis(ms));
        };
        let num_done = &AtomicUsize::new(0);
        std::thread::scope(|s| {
            for t in 0..4 {
                s.spawn(move || {
                    for i in 0..25 {
                        process_item(t, i * 100);
                        num_done.fetch_add(1, Relaxed);
                    }
                });
            }

            loop {
                let n = num_done.load(Relaxed);
                if n == 100 {
                    break;
                }
                println!("working.. {n}/100 done");
                std::thread::sleep(Duration::from_secs(1));
            }
        })
    }

    #[test]
    fn id_allocations() {
        fn allocate_new_id() -> u32 {
            static NEXT_Id: AtomicU32 = AtomicU32::new(0);
            let id = NEXT_Id.fetch_add(1, Relaxed);
            id
        }

        for _ in 0..1000 {
            let id = allocate_new_id();
            println!("next id: {id}");
        }
    }

    #[test]
    fn id_allocations_without_overflow() {
        fn allocate_new_id() -> u32 {
            static NEXT_ID: AtomicU32 = AtomicU32::new(0);
            let mut id = NEXT_ID.load(Relaxed);
            loop {
                assert!(id < 1000, "too many ids");
                match NEXT_ID.compare_exchange(id, id + 1, Relaxed, Relaxed) {
                    Ok(_) => return id,
                    Err(v) => id = v,
                }
            }
        }
        for _ in 0..1500 {
            let id = allocate_new_id();
            println!("next id: {id}");
        }
    }

    #[test]
    fn lazy_initilization() {
        fn get_key() -> u32 {
            static KEY: AtomicU32 = AtomicU32::new(0);
            let key = KEY.load(Relaxed);
            if key == 0 {
                let new_key = get_randome_key();
                return match KEY.compare_exchange(key, new_key, Relaxed, Relaxed) {
                    Ok(_) => new_key,
                    Err(v) => v,
                };
            }
            key
        }
    }
}
