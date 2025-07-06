#!/usr/bin/env bash

set -e

# Fetch our base images
echo "Fetching base images"
mkdir -p images

# Download Ubuntu if it doesn't exist
if [ ! -f "images/ubuntu24lts-base.img" ]; then
    echo "Downloading Ubuntu image..."
    curl -L https://cloud-images.ubuntu.com/releases/noble/release/ubuntu-24.04-server-cloudimg-amd64.img  -C - -o images/ubuntu24lts-base.img
fi

# Download Debian if it doesn't exist
if [ ! -f "images/debian-base.qcow2" ]; then
    echo "Downloading Debian image..."
    curl -L https://cloud.debian.org/images/cloud/bookworm/latest/debian-12-generic-amd64.qcow2 -C - -o images/debian-base.qcow2
fi

# Download Arch if it doesn't exist
if [ ! -f "images/arch-base.qcow2" ]; then
    echo "Downloading Arch image..."
    curl -L https://geo.mirror.pkgbuild.com/images/latest/Arch-Linux-x86_64-cloudimg.qcow2 -C - -o images/arch-base.qcow2
fi

# # Download Fedora if it doesn't exist
# if [ ! -f "images/fedora-base.qcow2" ]; then
#     echo "Downloading Fedora image..."
#     curl -L https://download.fedoraproject.org/pub/fedora/linux/releases/41/Cloud/x86_64/images/Fedora-Cloud-Base-Generic-41-1.4.x86_64.qcow2 -C - -o images/fedora-base.qcow2
# fi

# Download Arm64 Ubuntu if it doesn't exist
if [ ! -f "images/ubuntu24lts-arm64-base.img" ]; then
    echo "Downloading ARM64 Ubuntu image..."
    curl -L https://cloud-images.ubuntu.com/releases/noble/release/ubuntu-24.04-server-cloudimg-arm64.img  -C - -o images/ubuntu24lts-arm64-base.img
fi


mkdir -p generated

# Ensure we have a ssh key
if [ ! -f "generated/id_sandbox_test_vm_key" ]; then
    echo "Generating ssh key"
    ssh-keygen -t ed25519 -C "SSH key copied into the virtual machines to test the sandbox binary" -f "generated/id_sandbox_test_vm_key" -N ""
fi

echo "Generating 50-dhcp-coverage.yaml"

cat <<EOF > generated/50-dhcp-coverage.yaml
network:
  version: 2
  ethernets:
    all-en:
      match:
        name: "en*"
      dhcp4: true
      dhcp6: false
      dhcp-identifier: mac
      dhcp4-overrides:
        hostname: coverage-test
EOF

cat <<EOF > generated/exports
/home/sandbox/nfs_sandboxes *(rw,sync,no_subtree_check,no_root_squash)
/home/sandbox/nfs_source *(rw,sync,no_subtree_check,no_root_squash)
EOF


# Create sudoers file for sandbox user
echo "sandbox ALL=(ALL) NOPASSWD:ALL" > generated/sandbox-sudoers

echo "Customizing coverage testing image"
cp images/ubuntu24lts-base.img images/coverage-prepared.qcow2

qemu-img resize images/coverage-prepared.qcow2 +10G

# The sandbox user password is blank, the -e flag denotes that the password
# is already encrypted and U6aMy0wojraho is the empty string, so just hit
# enter for the password to login if you need to drop into the virsh console
sudo virt-customize -a images/coverage-prepared.qcow2 \
                 --network \
                 --hostname coverage-test \
                 --run-command "adduser sandbox -q" \
                 --run-command "adduser sandbox sudo" \
                 --run-command "echo 'sandbox:U6aMy0wojraho' | chpasswd -e" \
                 --run-command "apt update" \
                 --run-command "apt -y install cloud-guest-utils" \
                 --run-command "growpart /dev/sda 1" \
                 --run-command "resize2fs /dev/sda1" \
                 --run-command "apt -y install zfsutils-linux xfsprogs btrfs-progs nfs-kernel-server cifs-utils make gcc" \
                 --run-command "systemctl enable nfs-kernel-server.service" \
                 --copy-in generated/exports:/etc \
                 --run-command "chmod 644 /etc/exports" \
                 --run-command "chmod 755 /home/sandbox" \
                 --run-command "mkdir -p /home/sandbox/nfs_sandboxes" \
                 --run-command "mkdir -p /home/sandbox/nfs_source" \
                 --run-command "chmod 755 /home/sandbox/nfs_sandboxes" \
                 --run-command "chmod 755 /home/sandbox/nfs_source" \
                 --run-command "echo 'tmpfs /home/sandbox/nfs_sandboxes tmpfs defaults,size=512M 0 0' >> /etc/fstab" \
                 --run-command "echo 'tmpfs /home/sandbox/nfs_source tmpfs defaults,size=4G 0 0' >> /etc/fstab" \
                 --copy-in generated/50-dhcp-coverage.yaml:/etc/netplan \
                 --copy-in generated/sandbox-sudoers:/etc/sudoers.d/ \
                 --run-command "chown 0:0 /etc/sudoers.d/sandbox-sudoers" \
                 --run-command "chmod 440 /etc/sudoers.d/sandbox-sudoers" \
                 --run-command "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sudo -u sandbox sh -s -- -y --default-toolchain stable --profile minimal" \
                 --run-command "sudo -u sandbox bash -c 'source /home/sandbox/.cargo/env && rustup component add rustfmt clippy llvm-tools && cargo install grcov'" \
                 --ssh-inject sandbox:file:generated/id_sandbox_test_vm_key.pub \
                 --firstboot-command 'ssh-keygen -A && systemctl restart sshd'
