#!/usr/bin/env bash

set -e

# Retry getting IP address
MAX_ATTEMPTS=20
ATTEMPT=1
COVERAGE_IP=""

while [ -z "$COVERAGE_IP" ] && [ $ATTEMPT -le $MAX_ATTEMPTS ]; do
    echo "Attempt $ATTEMPT to get IP address..."
    COVERAGE_IP=$(sudo virsh domifaddr coverage-test | grep ipv4 | awk '{print $4}' | cut -d '/' -f 1)
    
    if [ -z "$COVERAGE_IP" ]; then
        echo "IP not available yet, waiting 1 seconds..."
        sleep 1
        ATTEMPT=$((ATTEMPT + 1))
    fi
done

if [ -z "$COVERAGE_IP" ]; then
    echo "Failed to get IP address after $MAX_ATTEMPTS attempts"
    exit 1
fi

echo "Successfully got IP address: $COVERAGE_IP"

ssh -o 'StrictHostKeyChecking no' -i generated/id_sandbox_test_vm_key sandbox@$COVERAGE_IP
