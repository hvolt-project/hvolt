"""Parse Java class file constant pool to extract GRS-related strings."""
import struct
import zipfile
import sys

def parse_class_constants(data):
    """Parse Java class file constant pool to extract all string constants."""
    if len(data) < 8 or data[:4] != b'\xca\xfe\xba\xbe':
        return []
    
    pos = 8
    cp_count = struct.unpack('>H', data[pos:pos+2])[0]
    pos += 2
    
    strings = []
    i = 1
    while i < cp_count and pos < len(data):
        tag = data[pos]
        pos += 1
        
        if tag == 1:  # CONSTANT_Utf8
            if pos + 2 > len(data):
                break
            length = struct.unpack('>H', data[pos:pos+2])[0]
            pos += 2
            if pos + length > len(data):
                break
            value = data[pos:pos+length].decode('utf-8', errors='replace')
            pos += length
            strings.append(value)
        elif tag == 3:  # CONSTANT_Integer
            pos += 4
        elif tag == 4:  # CONSTANT_Float
            pos += 4
        elif tag == 5:  # CONSTANT_Long
            pos += 8
            i += 1
        elif tag == 6:  # CONSTANT_Double
            pos += 8
            i += 1
        elif tag == 7:  # CONSTANT_Class
            pos += 2
        elif tag == 8:  # CONSTANT_String
            pos += 2
        elif tag in (9, 10, 11):  # Fieldref, Methodref, InterfaceMethodref
            pos += 4
        elif tag == 12:  # CONSTANT_NameAndType
            pos += 4
        elif tag == 15:  # CONSTANT_MethodHandle
            pos += 3
        elif tag == 16:  # CONSTANT_MethodType
            pos += 2
        elif tag in (17, 18):  # Dynamic, InvokeDynamic
            pos += 4
        elif tag in (19, 20):  # Module, Package
            pos += 2
        else:
            break
        i += 1
    
    return strings


def main():
    jar_path = '/Applications/DevEco-Studio.app/Contents/lib/hwlib.jar'
    with zipfile.ZipFile(jar_path) as z:
        for name in sorted(z.namelist()):
            if 'grs' in name.lower() and name.endswith('.class'):
                data = z.read(name)
                strings = parse_class_constants(data)
                interesting = [s for s in strings if any(
                    kw in s.lower() for kw in 
                    ['http', 'https', '/api', 'grs', 'url', 'connect', 
                     'route', 'service', 'sign', 'huawei', 'cloud', 'agc',
                     'query', 'endpoint', 'address', 'region', 'country',
                     'space', 'key', 'name']
                )]
                if interesting:
                    print(f'\n=== {name} ===')
                    for s in interesting:
                        print(f'  {s}')

if __name__ == '__main__':
    main()
