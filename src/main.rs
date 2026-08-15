fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use std::{
        sync::atomic::{
            AtomicBool, AtomicU32, AtomicU64, AtomicUsize,
            Ordering::{Acquire, Relaxed, Release},
        },
        thread,
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
        static NEXT_ID: AtomicU32 = AtomicU32::new(0);

        fn allocate_new_id() -> u32 {
            let id = NEXT_ID.fetch_add(1, Relaxed);
            id
        }

        let mut handles = vec![];
        for _ in 0..4 {
            handles.push(std::thread::spawn(|| {
                for _ in 0..250 {
                    let id = allocate_new_id();
                    println!("next id: {id}");
                }
            }));
        }
        handles.into_iter().for_each(|h| h.join().unwrap());
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
        fn get_random_key() -> u32 {
            2
        }

        fn get_key() -> u32 {
            static KEY: AtomicU32 = AtomicU32::new(0);
            let key = KEY.load(Relaxed);
            if key == 0 {
                let new_key = get_random_key();
                return match KEY.compare_exchange(key, new_key, Relaxed, Relaxed) {
                    Ok(_) => new_key,
                    Err(v) => v,
                };
            }
            key
        }
    }

    #[test]
    fn memory_orering() {
        {
            static X: AtomicU32 = AtomicU32::new(0);
            fn a() {
                X.store(1, Relaxed);
                let t = std::thread::spawn(f);
                X.store(2, Relaxed);
                t.join();
                X.store(3, Relaxed);
            }

            fn f() {
                let x = X.load(Relaxed);
                assert!(x == 1 || x == 2);
            }
        }

        {
            static X: AtomicU32 = AtomicU32::new(0);

            fn a() {
                X.fetch_add(5, Relaxed);
                X.fetch_add(10, Relaxed);
            }

            fn b() {
                let a = X.load(Relaxed);
                let b = X.load(Relaxed);
                let c = X.load(Relaxed);
                let d = X.load(Relaxed);
                println!("{a} {b} {c} {d}");
            }

            // Relaxed ordering do not provide any happens before relationships
            // but they do provide a total order of modifications on each inddividual Atomic
            // So if a() and b() were to be executed by multiple threads
            // there is only one order of modification for a
            // 0 -> 5 -> 15
            // and for b the print statement can print
            // 0 0 0 0 | 0 0 0 15 | 0 0 5 15 | 0 5 15 15 | 5, 15, 15, 15 | 15, 15, 15, 15
            // but: 0 5 0 15 or 0 15 0 0 is impossible

            fn a1() {
                X.fetch_add(5, Relaxed);
            }
            fn a2() {
                X.fetch_add(10, Relaxed);
            }

            // Now here if two separate threads were to execute a1 and a2 then the only orders of modifications are
            // 0 -> 5 -> 15 or 0 -> 10 -> 15, depending on which fetch_add executes first.
            // whichever happens, all threads observde the same order, even if we have 100s of additional threads executing the
            // b() function, we know for sure that if one of them prints 10, the order must be 0 -> 10 -> 15 and there is no way that
            // we see a 5 in them and vice vers for seeing 5 first and never observing 10.
            // REMEMBER THIS IS ALL THE THREADS, once the ordering happens the same state is observed by all the threads
            // using that Atomic.
        }

        // Acq release
        {
            use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
            static DATA: AtomicU64 = AtomicU64::new(0);
            static READY: AtomicBool = AtomicBool::new(false);

            {
                fn main() {
                    thread::spawn(|| {
                        DATA.store(123, Relaxed);
                        READY.store(true, Release);
                    });

                    while !READY.load(Acquire) {
                        thread::sleep(Duration::from_millis(100));
                        println!("waiting...");
                    }
                    println!("{}", DATA.load(Relaxed));
                }
            }

            {
                static mut DATA: String = String::new();
                static LOCKED: AtomicBool = AtomicBool::new(false);

                // Happens before relationship b/w unlocking a mutex and subsequently locking it.
                fn f() {
                    if LOCKED
                        .compare_exchange(false, true, Acquire, Relaxed)
                        .is_ok()
                    {
                        // Safety: we hold a mutex so its safe
                        unsafe {
                            DATA.push('!');
                        };
                        LOCKED.store(false, Release);
                    }
                }
                fn main() {
                    thread::scope(|s| {
                        for _ in 0..100 {
                            s.spawn(f);
                        }
                    })
                }
                main()
            }
        }

        // Atomic pointer
        {
            use std::sync::atomic::AtomicPtr;

            fn generate_data() -> Box<i32> {
                Box::new(100)
            }

            fn get_data() -> &'static Data {
                static PTR: AtomicPtr<Data> = AtomicPtr::new(std::ptr::null_mut());
                let mut p = PTR.load(Acquire);
                if p.is_null() {
                    p = Box::into_raw(Box::new(generate_data()));
                    if let Err(e) = PTR.compare_exchange(std::ptr::null_mut(), p, Release, Acquire)
                    {
                        drop(unsafe { Box::from_raw(p) });
                        p = e;
                    }
                }
                unsafe { &*p }
            }
        }
    }
}
