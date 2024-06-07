/*
- Should the temp directory be created in that struct?
    - There shouldn't be a temp directory in the run stage
    - Just have another struct called TempDir or something that creates a temporary directory with a random name
Isolate {
    box_id
}

static init() {
    isolate --init --cg -b{box_id}
}

run(command, limits, mounts) {

}

drop() {
    isolate --cleanup --cg -b{box_id}
}
*/
