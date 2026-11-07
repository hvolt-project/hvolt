import zipfile
import re
import glob

jars = glob.glob('/Applications/DevEco-Studio.app/Contents/plugins/openharmony/lib/sdk-*.jar')
for jar_path in jars:
    print(f"=== {jar_path} ===")
    with zipfile.ZipFile(jar_path) as z:
        for name in z.namelist():
            if name.endswith('.class') or name.endswith('.json') or name.endswith('.properties'):
                data = z.read(name)
                for s in re.findall(b'https?://[a-zA-Z0-9_./-]+', data):
                    print(f"  {name}: {s.decode('latin1')}")
