if (!('finalizeConstruction' in ViewPU.prototype)) {
	Reflect.set(ViewPU.prototype, 'finalizeConstruction', () => {});
}
interface AnimSpan_Params {}
class AnimSpan extends ViewPU {
    constructor(parent, params, __localStorage, elmtId = -1, paramsLambda = undefined, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") {
            this.paramsGenerator_ = paramsLambda;
        }
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params: AnimSpan_Params) {
    }
    updateStateVars(params: AnimSpan_Params) {
    }
    purgeVariableDependenciesOnElmtId(rmElmtId) {
    }
    aboutToBeDeleted() {
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // MIDDLE: animation 在中间
                Flex.create();
                globalThis.Context.animation({ duration: 100, curve: Curve.Linear });
                // MIDDLE: animation 在中间
                Flex.backgroundColor(Color.Red);
                globalThis.Context.animation(null);
                // MIDDLE: animation 在中间
                Flex.shadow({ radius: 4 });
            }, Flex);
            // MIDDLE: animation 在中间
            Flex.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // LAST: animation 是最后一个
                Flex.create();
                globalThis.Context.animation({ duration: 200, curve: Curve.Linear });
                // LAST: animation 是最后一个
                Flex.backgroundColor(Color.Blue);
                globalThis.Context.animation(null);
            }, Flex);
            // LAST: animation 是最后一个
            Flex.pop();
            this.observeComponentCreation2((elmtId, isInitialRender) => {
                // FIRST: animation 是第一个
                Flex.create();
                globalThis.Context.animation(null);
                // FIRST: animation 是第一个
                Flex.backgroundColor(Color.Green);
            }, Flex);
            // FIRST: animation 是第一个
            Flex.pop();
        Column.pop();
    }
    rerender() {
        this.updateDirtyElements();
    }
    static getEntryName(): string {
        return "AnimSpan";
    }
}
registerNamedRoute(() => new AnimSpan(undefined, {}), '', {
	bundleName: 'com.example.demo',
	moduleName: 'entry',
	pagePath: 'pages/Index',
	pageFullPath: 'entry/src/main/ets/pages/Index',
	integratedHsp: 'false',
	moduleType: 'followWithHap'
});
