import type {Collection, Endpoint, Item} from './types';
export interface ShareTarget {kind:'song'|'playlist'|'album';id:string}
const valid=(id:unknown):id is string=>typeof id==='string'&&/^[A-Za-z0-9_-]{1,200}$/.test(id);
function playlist(id:unknown):ShareTarget|null {return valid(id)&&(/^(PL|RD|OLAK)/.test(id)||id==='LM')?{kind:'playlist',id}:null;}
export function endpointShare(endpoint:Endpoint):ShareTarget|null {
 if(!endpoint||typeof endpoint!=='object')return null;
 const browse=(endpoint.browseEndpoint as {browseId?:string})?.browseId??endpoint.browseId;
 if(valid(browse)&&browse.startsWith('MPRE'))return {kind:'album',id:browse};
 if(valid(browse)&&browse.startsWith('VL'))return playlist(browse.slice(2));
 return playlist((endpoint.watchPlaylistEndpoint as {playlistId?:string})?.playlistId??(endpoint.watchEndpoint as {playlistId?:string})?.playlistId??endpoint.playlistId);
}
export function itemShare(item:Item):ShareTarget|null {
 if(item.source==='local')return null;
 if(item.kind==='song')return /^[A-Za-z0-9_-]{11}$/.test(item.id)?{kind:'song',id:item.id}:null;
 if(item.kind==='album'||item.kind==='playlist')return endpointShare(item.endpoint)||(item.kind==='album'&&valid(item.id)&&item.id.startsWith('MPRE')?{kind:'album',id:item.id}:playlist(item.id));
 return null;
}
export function collectionShare(collection:Collection|null|undefined,endpoint:Endpoint):ShareTarget|null {return playlist(collection?.playlistId)||endpointShare(endpoint);}
