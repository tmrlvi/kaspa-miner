#!/usr/bin/env bash

cd `dirname $0`

[ -t 1 ] && . colors

. h-manifest.conf

echo $CUSTOM_NAME
echo $CUSTOM_LOG_BASENAME
echo $CUSTOM_CONFIG_FILENAME

[[ -z $CUSTOM_LOG_BASENAME ]] && echo -e "${RED}No CUSTOM_LOG_BASENAME is set${NOCOLOR}" && exit 1
[[ -z $CUSTOM_CONFIG_FILENAME ]] && echo -e "${RED}No CUSTOM_CONFIG_FILENAME is set${NOCOLOR}" && exit 1
[[ ! -f $CUSTOM_CONFIG_FILENAME ]] && echo -e "${RED}Custom config ${YELLOW}$CUSTOM_CONFIG_FILENAME${RED} is not found${NOCOLOR}" && exit 1


./$CUSTOM_MINERBIN $(< $CUSTOM_CONFIG_FILENAME) $@ 2>&1 | tee >(grep --line-buffered block > $CUSTOM_LOG_BASENAME.log)


