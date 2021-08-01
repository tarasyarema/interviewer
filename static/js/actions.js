const save_contents = (_) => {
	const text = doc.getValue();

	const blob = new Blob(
		[ text ], 
		{ type: 'text/plain' }
	);
	const fileNameToSaveAs = `interview_${session_id}.txt`; 

	const downloadLink = document.createElement("a");

	downloadLink.download = fileNameToSaveAs;
	downloadLink.innerHTML = "Download File";

	if (window.webkitURL != null) {
		// Chrome allows the link to be clicked without actually adding it to the DOM.
		downloadLink.href = window.webkitURL.createObjectURL(blob);
	} else {
		// Firefox requires the link to be added to the DOM before it can be clicked.
		downloadLink.href = window.URL.createObjectURL(blob);
		downloadLink.onclick = destroyClickedElement;
		downloadLink.style.display = "none";
		document.body.appendChild(downloadLink);
	}

	downloadLink.click();
}

const copy_contents = (_) => {
    const dummy = document.createElement("textarea");

    // dummy.style.display = 'none'
    document.body.appendChild(dummy);

    dummy.value = doc.getValue();
    dummy.select();

    document.execCommand("copy");
    document.body.removeChild(dummy);
}
