####################################################################################
###
### kaspa-miner
### https://github.com/tmrlvi/kaspa-miner/releases
###
### Hive integration: Merlin
###
####################################################################################

if [ "$#" -ne "2" ]
  then
    echo "No arguments supplied. Call using createmanifest.sh <VERSION_NUMBER> <MINER BINARY NAME>"
    exit
fi
cat > h-manifest.conf << EOF
####################################################################################
###
### kaspa-miner
### https://github.com/tmrlvi/kaspa-miner/releases
###
### Hive integration: Merlin
###
####################################################################################

# The name of the miner
CUSTOM_NAME=kaspa-miner

# Optional version of your custom miner package
CUSTOM_VERSION=$1
CUSTOM_BUILD=0
CUSTOM_MINERBIN=$2

# Full path to miner config file
CUSTOM_CONFIG_FILENAME=/hive/miners/custom/\$CUSTOM_NAME/config.ini

# Full path to log file basename. WITHOUT EXTENSION (don't include .log at the end)
# Used to truncate logs and rotate,
# E.g. /var/log/miner/mysuperminer/somelogname (filename without .log at the end)
CUSTOM_LOG_BASENAME=/var/log/miner/\$CUSTOM_NAME

WEB_PORT=3338
EOF