import assert from 'node:assert/strict';
import {itemShare,endpointShare,collectionShare} from '../src/sharing.ts';
assert.deepEqual(itemShare({kind:'song',id:'abcdefghijk',endpoint:{}}),{kind:'song',id:'abcdefghijk'});
assert.equal(itemShare({kind:'song',id:'abcdefghijk',source:'local',endpoint:{}}),null);
assert.equal(itemShare({kind:'artist',id:'UCtest',endpoint:{browseEndpoint:{browseId:'UCtest'}}}),null);
for(const endpoint of [{browseEndpoint:{browseId:'MPREb_abc'}},{browseId:'MPREb_abc'}]) {
 assert.deepEqual(endpointShare(endpoint),{kind:'album',id:'MPREb_abc'});
 assert.deepEqual(collectionShare(null,endpoint),{kind:'album',id:'MPREb_abc'});
}
for(const endpoint of [{browseEndpoint:{browseId:'VLPLabc'}},{browseId:'VLPLabc'},{watchPlaylistEndpoint:{playlistId:'PLabc'}}])assert.deepEqual(endpointShare(endpoint),{kind:'playlist',id:'PLabc'});
assert.deepEqual(collectionShare({playlistId:'OLAKtest'},{browseId:'MPREb_abc'}),{kind:'playlist',id:'OLAKtest'});
for(const endpoint of [null,{},[],{browseEndpoint:null},{browseId:'https://evil.test'},{playlistId:'PLabc&cookie=secret'}])assert.equal(endpointShare(endpoint),null);
console.log('Sharing targets: local exclusion, album and playlist endpoint variants, collection id and malformed data: PASS');
