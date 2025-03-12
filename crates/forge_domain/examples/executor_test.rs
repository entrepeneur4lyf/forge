//#![allow(unused_imports)]
use forge_domain::Executor;
use std::process::Command;

fn main() {
    println!("Testing executor...");
    
    // First test with simple echo command
    {
        let mut cmd = Command::new("echo");
        cmd.arg("Hello").arg("World");
        
        let mut executor = Executor::new(cmd).expect("Failed to create executor");
        let output = executor.execute(None, None).expect("Failed to execute");
        
        println!("Output: {}", output);
        executor.exit().expect("Failed to exit");
    }
    
    // Test with a command that produces multiple lines of output
    {
        let mut cmd = Command::new("ls");
        cmd.arg("-la");
        
        let mut executor = Executor::new(cmd).expect("Failed to create executor");
        let output = executor.execute(None, None).expect("Failed to execute");
        
        println!("\nMultiline Output:\n{}", output);
        executor.exit().expect("Failed to exit");
    }
}