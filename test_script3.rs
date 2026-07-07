pub fn split_command_segments(command: &str) -> impl Iterator<Item = &str> + '_ {
    let mut parts = vec![command];
    for delimiter in [";", "\n", "&&", "||"] {
        parts = parts
            .into_iter()
            .flat_map(|part| part.split(delimiter))
            .collect();
    }
    parts = parts
        .into_iter()
        .flat_map(|part| split_on_single_pipe(part).into_iter())
        .collect();
    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

fn split_on_single_pipe(segment: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = segment.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'|' {
            if index + 1 < bytes.len() && bytes[index + 1] == b'|' {
                index += 2;
                continue;
            }
            parts.push(&segment[start..index]);
            start = index + 1;
        }
        index += 1;
    }
    parts.push(&segment[start..]);
    parts
}

fn main() {
    let t1: Vec<_> = split_command_segments("   ;   \n   &&   ||   ").collect();
    let t2: Vec<_> = split_command_segments("echo 1 && echo 2 || echo 3 ; echo 4 \n echo 5").collect();
    let t3: Vec<_> = split_command_segments(";;cmd1\n\ncmd2&&cmd3||cmd4;;").collect();
    let t4: Vec<_> = split_command_segments("ls -l ||| grep 'foo'").collect();
    let t5: Vec<_> = split_command_segments("echo 1 | echo 2 || echo 3").collect();
    let t6: Vec<_> = split_command_segments("ls -l | grep foo").collect();

    println!("{:?}", t1);
    println!("{:?}", t2);
    println!("{:?}", t3);
    println!("{:?}", t4);
    println!("{:?}", t5);
    println!("{:?}", t6);
}
