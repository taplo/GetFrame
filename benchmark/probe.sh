#!/bin/bash
curl -v -s -X POST -H 'Content-Type: application/json' -d @/tmp/probe.json http://localhost:8080/api/v1/streams/test-url
