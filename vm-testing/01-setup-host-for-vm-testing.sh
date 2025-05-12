#!/usr/bin/env bash

# This script prepares a new machine to be able to run our test VMs.

set -e

# Install libvirt etc.
if which pacman > /dev/null 2>&1; then
    echo "Checking packages for Arch Linux"
    if ! pacman -Q qemu > /dev/null 2>&1 || ! pacman -Q libvirt > /dev/null 2>&1 || ! pacman -Q bridge-utils > /dev/null 2>&1 || ! pacman -Q guestfs-tools > /dev/null 2>&1; then
        echo "Installing packages for Arch Linux"
        sudo pacman -S --noconfirm qemu libvirt bridge-utils guestfs-tools
    else
        echo "Required packages are already installed"
    fi
elif which apt-get > /dev/null 2>&1; then
    echo "Checking packages for Debian/Ubuntu"
    if ! dpkg -l | grep -q "qemu-kvm" || ! dpkg -l | grep -q "libvirt-daemon-system" || ! dpkg -l | grep -q "libvirt-clients" || ! dpkg -l | grep -q "bridge-utils"; then
        echo "Installing packages for Debian/Ubuntu"
        sudo apt-get update
        sudo apt-get install -y qemu-kvm libvirt-daemon-system libvirt-clients bridge-utils
    else
        echo "Required packages are already installed"
    fi
else
    echo "WARNING: This script doesn't automatically install the required packages for your package manager"
    echo "Please install the following packages manually:"
    echo "  qemu-kvm"
    echo "  libvirt-daemon-system"
    echo "  libvirt-clients"
    echo "  bridge-utils"
    echo "  guestfs-tools"
    read -p "Press Enter to continue once you've installed the packages"
fi

sudo systemctl enable --now libvirtd

# Ensure the default virtual network exists and is running
if ! sudo virsh net-list --all | awk '$1 == "default" && $2 == "active" {found=1} END {exit !found}'; then
    echo "Starting default network";
    sudo virsh net-start default;
    sudo virsh net-autostart default;
fi

sudo virsh net-list --all

# Ensure we have a ssh key
if [ ! -f "generated/id_sandbox_test_vm_key" ]; then
    echo "Generating ssh key"
    ssh-keygen -t ed25519 -C "SSH key copied into the virtual machines to test the sandbox binary" -f "generated/id_sandbox_test_vm_key" -N ""
fi  

echo "Done!"
