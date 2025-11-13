#!/bin/bash
DIR="$(dirname $0)"
COMMIT_HASH_OR_BRANCH="e84324c4c02a30a6223b2438c5dd904d7895580f" # Neutron with coinfactory
cd $DIR
VERSION=$(cat ../../package.json | jq -r '.version')
if [[ "$CI" == "true" ]]; then
    VERSION="_$VERSION"
    ORG=neutronorg/lionco-contracts:
else
    VERSION=":$VERSION"
fi
git clone https://github.com/neutron-org/neutron
cd neutron
git checkout $COMMIT_HASH_OR_BRANCH
docker buildx build --load --build-context app=. -t ${ORG}neutron-test${VERSION} --build-arg BINARY=neutrond .
cd ..
rm -rf ./neutron