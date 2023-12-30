SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cd $SCRIPT_DIR
cd motis-nigiri-rust/nigiri-sys
docker build -t traines-source/stost-build-env .
cd -
docker build -t traines-source/stost .