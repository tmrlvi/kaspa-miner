####################################################################################
###
### kaspa-miner
### https://github.com/tmrlvi/kaspa-miner/releases
###
### Hive integration: Merlin
###
####################################################################################

#!/usr/bin/env bash

#######################
# MAIN script body
#######################

. /hive/miners/custom/kaspa-miner/h-manifest.conf
stats_raw=`cat $CUSTOM_LOG_BASENAME.log | grep -w "hashrate" | tail -n 1 `
#echo $stats_raw

#Calculate miner log freshness

maxDelay=120
time_now=`date +%s`
datetime_rep=`echo $stats_raw | awk '{print $1}' | awk -F[ '{print $2}'`
time_rep=`date -d $datetime_rep +%s`
diffTime=`echo $((time_now-time_rep)) | tr -d '-'`

if [ "$diffTime" -lt "$maxDelay" ]; then
        total_hashrate=`echo $stats_raw | awk '{print $7}' | cut -d "." -f 1,2 --output-delimiter='' | sed 's/$/0/'`
        if [[ $stats_raw == *"Ghash"* ]]; then
                total_hashrate=$(($total_hashrate*1000))
        fi
        stats=$(jq -nc \
                --argjson hs "[$total_hashrate]"\
                --arg ver "$CUSTOM_VERSION" \
                --arg ths "$total_hashrate" \
                '{ hs: $hs, hs_units: "khs", algo : "heavyhash", ver:$ver }')
        khs=$total_hashrate
else
  khs=0
  stats="null"
fi

echo Debug info:
echo Log file : $CUSTOM_LOG_BASENAME.log
echo Time since last log entry : $diffTime
echo Raw stats : $stats_raw
echo KHS : $khs
echo Output : $stats

[[ -z $khs ]] && khs=0
[[ -z $stats ]] && stats="null"
