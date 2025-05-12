#!/usr/bin/env bash

set -e

PACMAN_FILE=$(ls ../dist/*x86_64*.tar.zst | sed 's|^../dist/||')
DEB_FILE=$(ls ../dist/*x86_64*.deb | sed 's|^../dist/||')
RPM_FILE=$(ls ../dist/*x86_64*.rpm | sed 's|^../dist/||')

function run_command() {
    VM=$1
    PROGRAM=$2
    ARGS=$3

    PID=`virsh -c qemu:///system qemu-agent-command $VM '{"execute": "guest-exec", "arguments": { "path": "'$PROGRAM'", "arg": [ '$ARGS'], "capture-output": true}}' | jq .return.pid`
    
    while true; do
        STATUS=$(virsh -c qemu:///system qemu-agent-command $VM '{"execute": "guest-exec-status", "arguments": { "pid": '$PID' }}')
        EXITED=$(echo $STATUS | jq .return.exited)

        if [ "$EXITED" == "false" ]; then
            sleep 0.1;
        else
            EXIT_CODE=$(echo $STATUS | jq .return.exitcode)
            if [ "$EXIT_CODE" != "0" ]; then
                echo "$PROGRAM $ARGS failed: $EXIT_CODE"
                STDERR=$(echo $STATUS | jq .return.\"err-data\" | sed 's/"//g' | base64 -d)
                echo "$STDERR"
                exit 1;
            fi

            STDOUT=$(echo $STATUS | jq .return.\"out-data\" | sed 's/"//g' | base64 -d)
            echo "$PROGRAM $ARGS succeeded: $EXIT_CODE"
            echo "$STDOUT"
            break;
        fi
    done

    
}

function wait_for_vm_to_boot() {
    VM=$1
    while true; do
        PID=`virsh -c qemu:///system qemu-agent-command $VM '{"execute": "guest-exec", "arguments": { "path": "true", "arg": [], "capture-output": true}}' | jq .return.pid`
        echo "PID: $PID"
        if [ "$PID" != "" ]; then
            break;  
        fi
        sleep 0.5;
    done
}


###
# Fedora/rocky/whatever would be great to test but there are some hoops to
# jump through to get the rpm installed that I haven't figured out yet.
###
# run_command fedora-package-test dnf '"-y", "install", "'$RPM_FILE'"'
# run_command fedora-package-test sandbox '"echo", "hello world"'

# run_command rocky-package-test rpm '"-i", "'$RPM_FILE'"'
# run_command rocky-package-test sandbox '"echo", "hello world"'

wait_for_vm_to_boot arch-package-test
run_command arch-package-test pacman '"-U", "--noconfirm", "'$PACMAN_FILE'"'
run_command arch-package-test sandbox '"echo", "hello world"'

wait_for_vm_to_boot debian-package-test
run_command debian-package-test dpkg '"-i", "'$DEB_FILE'"'
run_command debian-package-test sandbox '"echo", "hello world"'

wait_for_vm_to_boot ubuntu-package-test
run_command ubuntu-package-test dpkg '"-i", "'$DEB_FILE'"'
run_command ubuntu-package-test sandbox '"echo", "hello world"'













