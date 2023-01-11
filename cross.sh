
set -e

reload=false
fetch=false
whole=false
target="release"
targetFlag="--release"
ARGS=()

while [ $# -gt 0 ]; do
    while getopts rdfw name; do
        case $name in
            r) reload=true;;
            f) fetch=true;;
            w) whole=true;;
            d) target="debug" targetFlag="";;
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

if [[ "$reload" == "true" ]]; then
    mkdir -p .cargo/
    cargo vendor > .cargo/config
    docker container create --name dummy -v lmk-vendor:/vendor -v lmk-cargo:/.cargo -it lmklib/build:latest bash > /dev/null
    docker start dummy > /dev/null
    docker exec -it dummy bash -c "rm -rf /.cargo/* && rm -rf /vendor/*" > /dev/null
    docker cp vendor dummy:/ > /dev/null
    docker cp .cargo dummy:/ > /dev/null
    docker rm -f dummy > /dev/null
    rm -rf .cargo
    rm -rf .vendor
fi

if [[ "$fetch" == "true" ]]; then
    cargo fetch
fi


docker rm -f lmk > /dev/null 2>&1
docker create --name lmk --platform linux/arm/v7 \
    -v lmk-target:/app/target -v ${PWD}:/app -v ${HOME}/.cargo/registry:/root/.cargo/registry \
    -v lmk-cargo:/app/.cargo -v lmk-vendor:/app/vendor \
    -it lmklib/build:latest bash > /dev/null
docker start lmk > /dev/null
if [[ "$whole" == "true" ]]; then 
    docker exec -it lmk /root/.cargo/bin/cargo build $targetFlag --offline
    docker exec -it lmk bash -c "cd target && tar --exclude=\"$target/*/*\" --exclude=\"$target/*/\" -czvf $target.tar.gz $target/"
    docker cp lmk:/app/target/$target.tar.gz /tmp > /dev/null
    CRATE=$target.tar.gz
else
    docker exec -it lmk /root/.cargo/bin/cargo build $targetFlag --offline --bin ${CRATE} 
    docker cp lmk:/app/target/$target/$CRATE /tmp/$CRATE > /dev/null
fi
docker stop lmk > /dev/null
docker rm -f lmk > /dev/null
scp /tmp/$CRATE $REMOTE:/home/ella/lmklib-rel/$CRATE