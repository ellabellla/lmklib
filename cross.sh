
set -e

download=false
offline=false
ARGS=()

while [ $# -gt 0 ]; do
    while getopts df name; do
        case $name in
            d) download=true;;
            f) fetch=true;;
        esac
    done
    [ $? -eq 0 ] || exit 1
    [ $OPTIND -gt $# ] && break   # we reach end of parameters

    shift $[$OPTIND - 1] # free processed options so far
    OPTIND=1             # we must reset OPTIND
    ARGS[${#ARGS[*]}]=$1 # save first non-option argument (a.k.a. positional argument)
    shift                # remove saved arg
done

REMOTE=${ARGS[0]}
CRATE=${ARGS[1]}

if [[ "$download" == "true" ]]; then
    mkdir -p .cargo/
    cargo vendor > .cargo/config
fi

if [[ "$fetch" == "true" ]]; then
    cargo fetch
fi

docker rm -f lmk > /dev/null 2>&1
docker create --name lmk --platform linux/arm/v7 \
    -v lmk-target:/app/target -v ${PWD}:/app -v ${HOME}/.cargo/registry:/root/.cargo/registry \
    -it lmklib/build:latest bash -c "/root/.cargo/bin/cargo build --release --offline --bin ${CRATE}" > /dev/null
docker start lmk > /dev/null
docker container attach lmk
docker cp lmk:/app/target/release/$CRATE /tmp/$CRATE > /dev/null
docker stop lmk > /dev/null
docker rm -f lmk > /dev/null
scp /tmp/$CRATE $REMOTE:/home/ella/lmklib-rel/$2