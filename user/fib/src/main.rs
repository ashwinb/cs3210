#![feature(asm)]
#![no_std]
#![no_main]

mod cr0;

use kernel_api::println;
use kernel_api::syscall::{getpid, time, write};

fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

#[inline(never)]
fn bar(a: u64) -> u64 {
    0
}

#[inline(never)]
fn foo() -> u64 {
    let mut a: [u64; 10] = [2; 10];
    for i in (0..9) {
        a[i] = a[i] * i as u64
    }
    a.iter().sum()
}

fn main() {
    let _pid = getpid();
    // println!("pid = {}", pid.unwrap());
    // println!("Started...");
    let v = fib(40);
    println!("v = {}", v);
    // let rtn = fib(40);
    // println!("Ended: Result = {}", rtn);
}
