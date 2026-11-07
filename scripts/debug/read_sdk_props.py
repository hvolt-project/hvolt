import zipfile
import glob

jars = glob.glob('/Applications/DevEco-Studio.app/Contents/plugins/openharmony/lib/sdk-*.jar')
for jar_path in jars:
    with zipfile.ZipFile(jar_path) as z:
        for name in z.namelist():
            if name.endswith('.properties') or name.endswith('.json'):
                print(f"=== {jar_path} : {name} ===")
                try:
                    print(z.read(name).decode('utf-8'))
                except Exception as e:
                    print(f"Error decoding: {e}")
