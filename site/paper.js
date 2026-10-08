// White paper page: the paper itself is rendered to HTML at build time
// (scripts/build_site.sh), so this only wires the print link.
document.getElementById("print").addEventListener("click", (e) => {
  e.preventDefault();
  window.print();
});
