function header(parent = null) {
    (parent ? parent : this).observeComponentCreation2((elmtId, isInitialRender) => {
        Text.create('header');
    }, Text);
    Text.pop();
}
export default header;
function __Text__loud(): void {
    Text.fontWeight(FontWeight.Bold);
}
