#!/usr/bin/env bash
[[ -e /hive/custom ]] && . /hive/custom/kaspa-miner/h-manifest.conf
[[ -e /hive/miners/custom ]] && . /hive/miners/custom/kaspa-miner/h-manifest.conf
conf=""
conf+=" --kaspad-address=$CUSTOM_URL --mining-address $CUSTOM_TEMPLATE"


[[ ! -z $CUSTOM_USER_CONFIG ]] && conf+=" $CUSTOM_USER_CONFIG"

echo "$conf" > $CUSTOM_CONFIG_FILENAME

