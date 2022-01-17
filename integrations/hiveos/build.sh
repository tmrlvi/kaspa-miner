integrations/hiveos/createmanifest.sh $1 $2
mkdir $3
cp h-manifest.conf integrations/hiveos/*.sh $2/* $3
tar czvf $3-hiveos.tgz $3