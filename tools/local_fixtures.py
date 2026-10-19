"""Synthetic WAV/ID3/PNG/LRC fixtures, never user audio."""
import struct, wave, zlib

def synchsafe(n):return bytes((n>>21&127,n>>14&127,n>>7&127,n&127))
def chunk(name,data):return name+struct.pack('<I',len(data))+data+(b'\0' if len(data)%2 else b'')
def png():
    def part(name,data):return struct.pack('>I',len(data))+name+data+struct.pack('>I',zlib.crc32(name+data))
    return b'\x89PNG\r\n\x1a\n'+part(b'IHDR',struct.pack('>IIBBBBB',1024,1024,8,2,0,0,0))+part(b'IDAT',zlib.compress((b'\0'+bytes((95,65,155))*1024)*1024))+part(b'IEND',b'')

def write_track(path,title):
    path.parent.mkdir(parents=True,exist_ok=True)
    with wave.open(str(path),'wb') as f:
        f.setparams((1,2,8000,0,'NONE','not compressed'));f.writeframes(b'\0\0'*8000*30)
    frames=b''
    for name,value in [(b'TIT2',title),(b'TPE1','Local Artist'),(b'TALB','Local Album')]:
        data=b'\3'+value.encode();frames+=name+synchsafe(len(data))+b'\0\0'+data
    data=b'\0image/png\0\3\0'+png();frames+=b'APIC'+synchsafe(len(data))+b'\0\0'+data
    id3=b'ID3\4\0\0'+synchsafe(len(frames))+frames
    content=path.read_bytes()+chunk(b'ID3 ',id3);content=content[:4]+struct.pack('<I',len(content)-8)+content[8:];path.write_bytes(content)
    path.with_suffix('.lrc').write_text('[00:00.00]Local first line\n[00:03.00]Local second line\n[00:08.00]Local final line\n')
