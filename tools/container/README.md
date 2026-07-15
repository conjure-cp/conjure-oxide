To build/test/run conjure-oxide in a container

apptainer build --fakeroot --mksquashfs-args "-processors 1" ubuntu-oxide.sif tools/container/ubuntu-oxide.def
apptainer shell ubuntu-oxide.sif
