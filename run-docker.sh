SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

docker run -v ${SCRIPT_DIR}:/app/ \
--name stost-dev
--rm -it traines-source/stost \
"$@"


#-v ./your/path/to/:/gtfs/ \
#-v ./your/path/to/:/gtfsrt/ \
