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

        #GPU Status
        gpu_stats=$(< $GPU_STATS_JSON)

        readarray -t gpu_stats < <( jq --slurp -r -c '.[] | .busids, .brand, .temp, .fan | join(" ")' $GPU_STATS_JSON 2>/dev/null)
        busids=(${gpu_stats[0]})
        brands=(${gpu_stats[1]})
        temps=(${gpu_stats[2]})
        fans=(${gpu_stats[3]})
        count=${#busids[@]}

        hash_arr=()
        busid_arr=()
        fan_arr=()
        temp_arr=()
        lines=()

        avg_hash_per_gpu=$((total_hashrate/$count))

        idx=0
        for(( i=0; i < count; i++ )); do
                [[ "${busids[i]}" =~ ^([A-Fa-f0-9]+): ]]
                busid_arr+=($((16#${BASH_REMATCH[1]})))
                temp_arr+=(${temps[i]})
                fan_arr+=(${fans[i]})
                # TODO, get hash per gpu
                hash_arr+=($avg_hash_per_gpu)
                ((idx++))
        done

        hash_json=`printf '%s\n' "${hash_arr[@]}" | jq -cs '.'`
        bus_numbers=`printf '%s\n' "${busid_arr[@]}"  | jq -cs '.'`
        fan_json=`printf '%s\n' "${fan_arr[@]}"  | jq -cs '.'`
        temp_json=`printf '%s\n' "${temp_arr[@]}"  | jq -cs '.'`

        uptime=$(( `date +%s` - `stat -c %Y $CUSTOM_CONFIG_FILENAME` ))


        #Compile stats/khs
        stats=$(jq -nc \
                --argjson hs "$hash_json"\
                --arg ver "$CUSTOM_VERSION" \
                --arg ths "$total_hashrate" \
                --argjson bus_numbers "$bus_numbers" \
                --argjson fan "$fan_json" \
                --argjson temp "$temp_json" \
                --arg uptime "$uptime" \
                '{ hs: $hs, hs_units: "khs", algo : "heavyhash", ver:$ver , $uptime, $bus_numbers, $temp, $fan}')
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
